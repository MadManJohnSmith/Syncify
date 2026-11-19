//! Issue #95 — refresco de credenciales: proactivo con vencimiento y reactivo
//! al 401/403.
//!
//! Contrato: ADR-0007 decisiones 12 y 14. Todos los tests usan reloj falso
//! (`now` inyectado) y transportes falsos con tokens fabricados: cero red.
//! Las bases son `:memory:` completamente migradas (la puerta de integridad
//! de sqlx aplica).
//!
//! Criterios del issue cubiertos aquí:
//! - Con vencimiento conocido, se refresca antes de usar la credencial.
//! - Sin vencimiento conocido, se refresca al primer 401/403.
//! - Un refresco fallido marca reautenticación y no reintenta en bucle.
//! - El refresco no duplica cuentas ni pierde el artefacto anterior si falla.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use syncify_tauri_lib::commands::account_refresh::{
    apply_refreshed_credentials, handle_credential_rejection, refresh_account_credential,
    tidal_refresh_payload, CredentialRefreshTransport, RefreshError, RefreshOutcome,
    RejectionOutcome,
};
use syncify_tauri_lib::crypto;

/// Reloj falso compartido: una época arbitraria y estable.
const NOW: i64 = 1_800_000_000;

/// Transporte falso: cuenta los intentos y devuelve el resultado programado.
/// La clave determinista de la suite: el número de llamadas es la evidencia
/// de que no hay bucle de reintentos.
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

    fn err(error: RefreshError) -> Self {
        Self {
            outcome: Err(error),
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
    pool
}

async fn service_id(pool: &Pool<Sqlite>, service: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("servicio sembrado por las migraciones")
}

/// Inserta una cuenta con el payload de credenciales ya CIFRADO.
/// `creds_json = None` deja la columna a NULL (cuenta sin sesión).
async fn insert_account(
    pool: &Pool<Sqlite>,
    service: &str,
    creds: Option<&serde_json::Value>,
) -> i64 {
    let sid = service_id(pool, service).await;
    let encrypted = creds.map(|payload| {
        crypto::encrypt(&payload.to_string()).expect("cifrado de credenciales de prueba")
    });
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email, is_active, credentials_json) \
         VALUES (?, ?, ?, 1, ?) RETURNING id",
    )
    .bind(sid)
    .bind(format!("Cuenta {service}"))
    .bind(format!("acct-{service}@example.invalid"))
    .bind(&encrypted)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

async fn stored_credentials(pool: &Pool<Sqlite>, account_id: i64) -> Option<serde_json::Value> {
    let encrypted: Option<String> =
        sqlx::query_scalar("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(pool)
            .await
            .expect("fila de cuenta");
    encrypted.map(|blob| {
        let plain = crypto::decrypt(&blob).expect("credencial legible");
        serde_json::from_str(&plain).expect("credencial JSON")
    })
}

async fn blob_credentials(pool: &Pool<Sqlite>, account_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT credentials_json FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("fila de cuenta")
}

async fn invalid_flag(pool: &Pool<Sqlite>, account_id: i64) -> bool {
    let flag: i64 =
        sqlx::query_scalar("SELECT IFNULL(credentials_invalid, 0) FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(pool)
            .await
            .expect("fila de cuenta");
    flag != 0
}

async fn invalid_reason(pool: &Pool<Sqlite>, account_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT invalid_reason FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("fila de cuenta")
}

async fn account_count(pool: &Pool<Sqlite>, service: &str) -> i64 {
    let sid = service_id(pool, service).await;
    sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(sid)
        .fetch_one(pool)
        .await
        .expect("recuento de cuentas")
}

// ═══════════════════════════════════════════════════════
// Criterio 1: con vencimiento conocido, se refresca antes de usar
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn con_vencimiento_vencido_se_refresca_antes_de_usar() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "spotify",
        Some(&serde_json::json!({
            "access_token": "viejo",
            "refresh_token": "rt",
            "expires_at": NOW - 10,
        })),
    )
    .await;

    let transport = FakeTransport::ok(serde_json::json!({
        "token_type": "Bearer",
        "access_token": "nuevo",
        "refresh_token": "rt",
        "expires_at": NOW + 3600,
    }));

    let outcome = refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("refresco proactivo");

    assert_eq!(outcome, RefreshOutcome::Refreshed);
    assert_eq!(transport.call_count(), 1);

    let creds = stored_credentials(&pool, account)
        .await
        .expect("credencial");
    assert_eq!(creds["access_token"], "nuevo");
    assert_eq!(creds["expires_at"], NOW + 3600);
    assert!(
        !invalid_flag(&pool, account).await,
        "refresco exitoso no marca"
    );
}

/// Dentro del buffer proactivo (300 s) también se refresca antes de usar.
#[tokio::test]
async fn dentro_del_buffer_proactivo_se_refresca() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "tidal",
        Some(&serde_json::json!({
            "access_token": "viejo",
            "token_expiry": (NOW + 299) as f64,
        })),
    )
    .await;

    let transport = FakeTransport::ok(serde_json::json!({ "access_token": "nuevo" }));
    let outcome = refresh_account_credential(&pool, "tidal", account, &transport, NOW)
        .await
        .expect("refresco proactivo");

    assert_eq!(outcome, RefreshOutcome::Refreshed);
    assert_eq!(transport.call_count(), 1);
}

/// Vencimiento lejano: no se refresca todavía.
#[tokio::test]
async fn con_vencimiento_fresco_no_se_refresca() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "spotify",
        Some(&serde_json::json!({ "access_token": "vivo", "expires_at": NOW + 3600 })),
    )
    .await;

    let transport = FakeTransport::ok(serde_json::json!({ "access_token": "x" }));
    let outcome = refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("decisión de confianza");

    assert_eq!(outcome, RefreshOutcome::NotNeeded);
    assert_eq!(transport.call_count(), 0, "no hay intento sin vencer");
}

/// Sin vencimiento publicado NO hay refresco proactivo: el camino contratado
/// es reactivo al primer 401/403.
#[tokio::test]
async fn sin_vencimiento_no_se_refresca_proactivo() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "qobuz",
        Some(&serde_json::json!({ "user_auth_token": "q-token" })),
    )
    .await;

    let transport = FakeTransport::ok(serde_json::json!({ "access_token": "x" }));
    let outcome = refresh_account_credential(&pool, "qobuz", account, &transport, NOW)
        .await
        .expect("decisión de confianza");

    assert_eq!(outcome, RefreshOutcome::NotNeeded);
    assert_eq!(transport.call_count(), 0);
}

// ═══════════════════════════════════════════════════════
// Criterio 2: sin vencimiento conocido, se refresca al primer 401/403
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn sin_vencimiento_se_refresca_al_primer_401() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "soundcloud",
        Some(&serde_json::json!({ "oauth_token": "sc-viejo" })),
    )
    .await;

    let transport = FakeTransport::ok(serde_json::json!({ "oauth_token": "sc-nuevo" }));
    let outcome = handle_credential_rejection(
        &pool,
        "soundcloud",
        account,
        401,
        &transport,
        "HTTP 401: token rejected",
    )
    .await
    .expect("refresco reactivo");

    assert_eq!(outcome, RejectionOutcome::Refreshed);
    assert_eq!(transport.call_count(), 1, "un intento en el PRIMER rechazo");
    assert!(!invalid_flag(&pool, account).await);

    let creds = stored_credentials(&pool, account)
        .await
        .expect("credencial");
    assert_eq!(creds["oauth_token"], "sc-nuevo");
}

#[tokio::test]
async fn sin_vencimiento_se_refresca_al_primer_403() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "qobuz",
        Some(&serde_json::json!({ "user_auth_token": "q-viejo" })),
    )
    .await;

    let transport = FakeTransport::ok(serde_json::json!({ "user_auth_token": "q-nuevo" }));
    let outcome = handle_credential_rejection(&pool, "qobuz", account, 403, &transport, "HTTP 403")
        .await
        .expect("refresco reactivo");

    assert_eq!(outcome, RejectionOutcome::Refreshed);
    assert_eq!(transport.call_count(), 1);
}

// ═══════════════════════════════════════════════════════
// Criterio 3: un refresco fallido marca reautenticación y no reintenta
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn refresco_reactivo_rechazado_marca_reautenticacion_sin_bucle() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "soundcloud",
        Some(&serde_json::json!({ "oauth_token": "sc-viejo" })),
    )
    .await;
    let original = blob_credentials(&pool, account)
        .await
        .expect("blob original");

    let transport = FakeTransport::err(RefreshError::ProviderRejected(
        "invalid_grant: refresh token rejected".to_string(),
    ));
    let outcome =
        handle_credential_rejection(&pool, "soundcloud", account, 401, &transport, "HTTP 401")
            .await
            .expect("manejo reactivo");

    assert_eq!(outcome, RejectionOutcome::MarkedForReauth);
    assert_eq!(transport.call_count(), 1);
    assert!(invalid_flag(&pool, account).await);
    assert!(invalid_reason(&pool, account).await.is_some());

    // El artefacto anterior NO se pierde: el fallo no escribe credenciales.
    assert_eq!(
        blob_credentials(&pool, account)
            .await
            .expect("blob tras fallo"),
        original,
        "el payload cifrado queda intacto tras el fallo"
    );

    // Segunda llamada con la cuenta ya marcada: sin intento nuevo (no bucle).
    let second =
        handle_credential_rejection(&pool, "soundcloud", account, 401, &transport, "HTTP 401")
            .await
            .expect("segunda llamada");
    assert_eq!(second, RejectionOutcome::AlreadyMarked);
    assert_eq!(
        transport.call_count(),
        1,
        "la cuenta marcada no recibe más intentos"
    );
}

#[tokio::test]
async fn refresco_proactivo_rechazado_marca_reautenticacion_sin_bucle() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "spotify",
        Some(&serde_json::json!({
            "access_token": "viejo",
            "refresh_token": "rt-muerto",
            "expires_at": NOW - 10,
        })),
    )
    .await;

    let transport = FakeTransport::err(RefreshError::ProviderRejected(
        "Token refresh failed: 400 invalid_grant".to_string(),
    ));
    let outcome = refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("refresco proactivo");

    assert_eq!(outcome, RefreshOutcome::MarkedForReauth);
    assert_eq!(transport.call_count(), 1);
    assert!(invalid_flag(&pool, account).await);

    // La puerta anti-bucle: el próximo uso ve la cuenta marcada y NO reintenta.
    let second = refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("segunda decisión");
    assert_eq!(second, RefreshOutcome::NeedsReauth);
    assert_eq!(transport.call_count(), 1);
}

/// Un refresco imposible (sin transporte programático, p. ej. Qobuz) también
/// marca reautenticación en el primer 401, sin bucle.
#[tokio::test]
async fn refresco_imposible_marca_reautenticacion() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "deezer",
        Some(&serde_json::json!({ "arl": "arl-deezer" })),
    )
    .await;

    let transport = FakeTransport::err(RefreshError::NotRefreshable(
        "no programmatic refresh for service 'deezer'".to_string(),
    ));
    let outcome =
        handle_credential_rejection(&pool, "deezer", account, 401, &transport, "HTTP 401")
            .await
            .expect("manejo reactivo");

    assert_eq!(outcome, RejectionOutcome::MarkedForReauth);
    assert!(invalid_flag(&pool, account).await);
}

/// Un fallo TEMPORAL (red, 5xx) NO marca la cuenta (decisión 12): una caída
/// del proveedor no es una credencial inválida, y el próximo uso reintenta.
#[tokio::test]
async fn fallo_temporal_no_marca_la_cuenta() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "spotify",
        Some(&serde_json::json!({
            "access_token": "viejo",
            "refresh_token": "rt",
            "expires_at": NOW - 10,
        })),
    )
    .await;

    let transport = FakeTransport::err(RefreshError::Transient(
        "Refresh request failed: connection refused".to_string(),
    ));
    let outcome = refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("refresco proactivo");

    assert_eq!(outcome, RefreshOutcome::RetryLater);
    assert!(!invalid_flag(&pool, account).await, "sin marca por caída");

    // El próximo uso reintenta de verdad: sin latch que lo impida.
    let second = refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("segundo intento");
    assert_eq!(second, RefreshOutcome::RetryLater);
    assert_eq!(transport.call_count(), 2, "el fallo temporal no enlaza");
}

// ═══════════════════════════════════════════════════════
// Criterio 4: el refresco no duplica cuentas ni pierde el artefacto
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn el_refresco_no_duplica_cuentas() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "spotify",
        Some(&serde_json::json!({
            "access_token": "viejo",
            "refresh_token": "rt",
            "expires_at": NOW - 10,
        })),
    )
    .await;
    let before = account_count(&pool, "spotify").await;

    let transport = FakeTransport::ok(serde_json::json!({
        "token_type": "Bearer",
        "access_token": "nuevo",
        "refresh_token": "rt",
        "expires_at": NOW + 3600,
    }));
    refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("refresco");

    assert_eq!(
        account_count(&pool, "spotify").await,
        before,
        "misma fila, sin duplicado"
    );
    let stored = stored_credentials(&pool, account)
        .await
        .expect("credencial");
    assert_eq!(
        stored["access_token"], "nuevo",
        "la MISMA fila recibió el token nuevo"
    );
}

/// La persistencia por fusión conserva campos que el transporte no conoce
/// (el `session_id` de Tidal debe sobrevivir al refresco).
#[tokio::test]
async fn el_payload_persistido_conserva_campos_ajenos_al_refresco() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "tidal",
        Some(&serde_json::json!({
            "access_token": "viejo",
            "refresh_token": "rt",
            "token_expiry": (NOW - 10) as f64,
            "session_id": "sesion-viva",
            "username": "cuenta",
        })),
    )
    .await;

    // El transporte falso simula el transporte de producción de Tidal:
    // fusiona el struct actualizado sobre el payload actual.
    struct TidalFakeTransport {
        new_token_expiry: f64,
        calls: AtomicUsize,
    }
    impl CredentialRefreshTransport for TidalFakeTransport {
        fn refresh<'a>(
            &'a self,
            _db: &'a sqlx::SqlitePool,
            _service: &'a str,
            _account_id: i64,
            creds: &'a serde_json::Value,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, RefreshError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let updated = syncify_tidal_downloader::TidalGuiCredentials {
                    access_token: "tidal-nuevo".to_string(),
                    refresh_token: Some("rt".to_string()),
                    token_expiry: Some(self.new_token_expiry),
                    expires_at: None,
                    expires_in: None,
                    user_id: None,
                    country_code: None,
                    client_id: None,
                    client_secret: None,
                    session_id: None,
                };
                tidal_refresh_payload(creds, &updated).map_err(RefreshError::Transient)
            })
        }
    }

    let transport = TidalFakeTransport {
        new_token_expiry: (NOW + 14400) as f64,
        calls: AtomicUsize::new(0),
    };
    let outcome = refresh_account_credential(&pool, "tidal", account, &transport, NOW)
        .await
        .expect("refresco proactivo tidal");

    assert_eq!(outcome, RefreshOutcome::Refreshed);
    let creds = stored_credentials(&pool, account)
        .await
        .expect("credencial");
    assert_eq!(creds["access_token"], "tidal-nuevo");
    assert_eq!(creds["token_expiry"], (NOW + 14400) as f64);
    assert_eq!(
        creds["session_id"], "sesion-viva",
        "el session_id sobrevive"
    );
    assert_eq!(creds["username"], "cuenta");
}

// ═══════════════════════════════════════════════════════
// Bordes: status no credencial, cuenta ya marcada, credenciales rotas
// ═══════════════════════════════════════════════════════

/// 5xx y demás NO son rechazos de credencial: sin escritura en BD ni
/// intento de refresco (decisión 12).
#[tokio::test]
async fn status_no_credencial_no_toca_nada() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "qobuz",
        Some(&serde_json::json!({ "user_auth_token": "q-token" })),
    )
    .await;

    for status in [500u16, 429, 200, 404] {
        let transport = FakeTransport::ok(serde_json::json!({ "access_token": "x" }));
        let outcome = handle_credential_rejection(
            &pool,
            "qobuz",
            account,
            status,
            &transport,
            "algo del proveedor",
        )
        .await
        .expect("manejo reactivo");
        assert_eq!(outcome, RejectionOutcome::NotCredentialRejection);
        assert_eq!(transport.call_count(), 0);
        assert!(!invalid_flag(&pool, account).await);
    }
}

/// Una cuenta ya marcada no recibe intentos ni re-emite el evento.
#[tokio::test]
async fn cuenta_ya_marcada_no_recibe_intentos() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "qobuz",
        Some(&serde_json::json!({ "user_auth_token": "q-token" })),
    )
    .await;
    sqlx::query("UPDATE accounts SET credentials_invalid = 1, invalid_reason = 'token_expired' WHERE id = ?")
        .bind(account)
        .execute(&pool)
        .await
        .expect("marca previa");

    let transport = FakeTransport::ok(serde_json::json!({ "user_auth_token": "nuevo" }));
    let reactive =
        handle_credential_rejection(&pool, "qobuz", account, 401, &transport, "HTTP 401")
            .await
            .expect("manejo reactivo");
    assert_eq!(reactive, RejectionOutcome::AlreadyMarked);

    let proactive = refresh_account_credential(&pool, "qobuz", account, &transport, NOW)
        .await
        .expect("decisión proactiva");
    assert_eq!(proactive, RefreshOutcome::NeedsReauth);

    assert_eq!(
        transport.call_count(),
        0,
        "cero intentos sobre cuenta marcada"
    );
}

/// Credencial indescifrable (decisión 10): se pide reautenticar, no se
/// inventan credenciales ni se falla en silencio.
#[tokio::test]
async fn credencial_indescifrable_marca_reautenticacion() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let sid = service_id(&pool, "spotify").await;
    sqlx::query("INSERT INTO accounts (service_id, display_name, email, is_active, credentials_json) VALUES (?, ?, ?, 1, ?) RETURNING id")
        .bind(sid)
        .bind("Rota")
        .bind("rota@example.invalid")
        .bind("no-es-un-blob-valido")
        .fetch_one(&pool)
        .await
        .expect("cuenta rota sembrada");

    let account: i64 =
        sqlx::query_scalar("SELECT id FROM accounts WHERE email = 'rota@example.invalid'")
            .fetch_one(&pool)
            .await
            .expect("id de cuenta rota");

    let transport = FakeTransport::ok(serde_json::json!({ "access_token": "x" }));
    let proactive = refresh_account_credential(&pool, "spotify", account, &transport, NOW)
        .await
        .expect("decisión proactiva");
    assert_eq!(proactive, RefreshOutcome::MarkedForReauth);

    let reactive =
        handle_credential_rejection(&pool, "spotify", account, 401, &transport, "HTTP 401")
            .await
            .expect("manejo reactivo");
    assert_eq!(
        reactive,
        RejectionOutcome::AlreadyMarked,
        "la marca previa corta el intento"
    );

    assert_eq!(transport.call_count(), 0);
}

/// Cuenta sin credencial almacenada: proactivamente no hay nada que refrescar;
/// reactivamente el 401 marca reautenticación (sin sesión que refrescar).
#[tokio::test]
async fn credencial_ausente_proactiva_confia_y_reactiva_marca() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(&pool, "qobuz", None).await;

    let transport = FakeTransport::ok(serde_json::json!({ "access_token": "x" }));
    let proactive = refresh_account_credential(&pool, "qobuz", account, &transport, NOW)
        .await
        .expect("decisión proactiva");
    assert_eq!(proactive, RefreshOutcome::NotNeeded);
    assert_eq!(transport.call_count(), 0);

    let reactive =
        handle_credential_rejection(&pool, "qobuz", account, 401, &transport, "HTTP 401")
            .await
            .expect("manejo reactivo");
    assert_eq!(reactive, RejectionOutcome::MarkedForReauth);
    assert!(invalid_flag(&pool, account).await);
    assert_eq!(
        transport.call_count(),
        0,
        "sin credencial no hay transporte posible"
    );
}

/// Cuenta inexistente o de otro servicio: error del llamador, no estado
/// silencioso.
#[tokio::test]
async fn cuenta_inexistente_o_de_otro_servicio_es_error() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "spotify",
        Some(&serde_json::json!({ "access_token": "t", "expires_at": NOW - 1 })),
    )
    .await;

    let transport = FakeTransport::ok(serde_json::json!({ "access_token": "x" }));

    assert!(
        refresh_account_credential(&pool, "spotify", 999_999, &transport, NOW)
            .await
            .is_err(),
        "cuenta inexistente es error"
    );
    assert!(
        refresh_account_credential(&pool, "qobuz", account, &transport, NOW)
            .await
            .is_err(),
        "servicio que no corresponde a la cuenta es error"
    );
}

/// La persistencia directa (usada por los transportes de producción que ya
/// resolvieron el token) escribe en la misma fila y levanta marcas previas.
#[tokio::test]
async fn apply_refreshed_credentials_actualiza_in_place_y_desmarca() {
    let _ = crypto::init_crypto([95u8; 32]);
    let pool = setup_db().await;
    let account = insert_account(
        &pool,
        "spotify",
        Some(&serde_json::json!({ "access_token": "viejo", "expires_at": NOW - 10 })),
    )
    .await;
    sqlx::query("UPDATE accounts SET credentials_invalid = 1, invalid_reason = 'token_expired', last_auth_error = 'x', last_auth_error_at = 'x' WHERE id = ?")
        .bind(account)
        .execute(&pool)
        .await
        .expect("marca previa");

    apply_refreshed_credentials(
        &pool,
        account,
        &serde_json::json!({ "access_token": "nuevo", "expires_at": NOW + 3600 }),
    )
    .await
    .expect("persistencia");

    assert!(!invalid_flag(&pool, account).await);
    assert!(invalid_reason(&pool, account).await.is_none());
    let creds = stored_credentials(&pool, account)
        .await
        .expect("credencial");
    assert_eq!(creds["access_token"], "nuevo");
}
