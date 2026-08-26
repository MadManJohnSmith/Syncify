//! Sprint S185 «Tidal RequiresAuth persistente» — pruebas de regresión.
//!
//! Cadena demostrada con evidencia de BD/logs del propietario:
//!   login OK → credenciales guardadas SIN timestamp de expiración
//!   → is_expired() las trata como vencidas en el primer uso
//!   → refresco OAuth falla por transporte (sin internet)
//!   → rama antigua marcaba credentials_invalid=1 PERSISTENTE
//!   → worker re-marcaba todas las filas del servicio ante el mensaje
//!     genérico «RequiresAuth: No active…»
//!   → RequiresAuth eterno pese a re-login.
//!
//! Estos tests fijan el contrato corregido:
//! 1. Credenciales activas y vigentes NUNCA se invalidan.
//! 2. Error de TRANSPORTE durante refresco NO escribe invalidación persistente.
//! 3. Expiración real sin refresh_token sí marca inválida (fin de línea genuino).
//! 4. El upsert de login deja exactamente el estado limpio que el pipeline selecciona.
//! 5. El payload Tidal sin expiración recibe la del cache o un fallback conservador.
//! 6. La clasificación del worker no amplifica mensajes genéricos RequiresAuth.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{inject_tidal_expiry, upsert_service_account};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::tidal_pipeline::resolve_and_refresh_gui_credentials;
use syncify_tauri_lib::worker::classify_session_auth_failure;

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Cliente HTTP cuyo tráfico muere instantáneamente contra un proxy local cerrado.
/// Simula de forma determinista un error de transporte (conexión rechazada),
/// exactamente la clase de fallo que envenenó la cuenta durante la ventana sin
/// internet — sin depender de que la máquina tenga o no red real.
fn dead_transport_client() -> reqwest::Client {
    reqwest::Client::builder()
        .proxy(reqwest::Proxy::all("http://127.0.0.1:1").expect("valid proxy url"))
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("reqwest client builds")
}

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn tidal_service_id(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(pool)
        .await
        .expect("tidal service seeded by migrations")
}

/// Inserta una fila de cuenta Tidal con credenciales cifradas y flags dados.
/// `email` debe ser único por servicio (schema: UNIQUE(service_id, email)).
async fn insert_tidal_account(
    pool: &sqlx::SqlitePool,
    email: &str,
    credentials_json: &str,
    credentials_invalid: bool,
    invalid_reason: Option<&str>,
) -> i64 {
    let encrypted = crypto::encrypt(credentials_json).expect("encrypt succeeds");
    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json,
                                 credentials_invalid, invalid_reason, last_auth_error, is_active)
           VALUES (?, 'S185 Tester', ?, ?, ?, ?, NULL, 1)
           RETURNING id"#,
    )
    .bind(tidal_service_id(pool).await)
    .bind(email)
    .bind(&encrypted)
    .bind(credentials_invalid as i64)
    .bind(invalid_reason)
    .fetch_one(pool)
    .await
    .expect("account insert succeeds")
}

async fn account_flags(pool: &sqlx::SqlitePool, account_id: i64) -> (i64, Option<String>) {
    sqlx::query_as::<_, (i64, Option<String>)>(
        "SELECT COALESCE(credentials_invalid, 0), invalid_reason FROM accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("account row exists")
}

#[tokio::test]
async fn test_s185_valid_active_credentials_resolve_without_invalidation() {
    let pool = setup_test_db().await;

    let creds = serde_json::json!({
        "access_token": "fresh_access_token",
        "refresh_token": "fresh_refresh_token",
        "token_expiry": now_secs() + 3600.0,
        "expires_at": now_secs() + 3600.0,
        "country_code": "US",
    });
    let account_id =
        insert_tidal_account(&pool, "valid@test.example", &creds.to_string(), false, None).await;

    let http_client = reqwest::Client::new();
    let (resolved, username) =
        resolve_and_refresh_gui_credentials(&pool, &http_client).await;

    assert!(
        resolved.is_some(),
        "un token vigente debe resolverse sin refresco ni red"
    );
    assert_eq!(resolved.unwrap().access_token, "fresh_access_token");
    assert_eq!(username.as_deref(), Some("S185 Tester"));

    let (inv, reason) = account_flags(&pool, account_id).await;
    assert_eq!(inv, 0, "una resolución exitosa no debe tocar el flag");
    assert!(reason.is_none());
}

#[tokio::test]
async fn test_s185_transport_error_during_refresh_does_not_persist_invalidation() {
    let pool = setup_test_db().await;

    // Token ya vencido con refresh_token presente: fuerza el intento de refresco,
    // que aquí fracasa por transporte (proxy muerto) — NO por rechazo de Tidal.
    let creds = serde_json::json!({
        "access_token": "expired_access_token",
        "refresh_token": "still_valid_refresh_token",
        "token_expiry": now_secs() - 100.0,
        "expires_at": now_secs() - 100.0,
        "country_code": "US",
    });
    let account_id =
        insert_tidal_account(&pool, "transport@test.example", &creds.to_string(), false, None).await;

    let http_client = dead_transport_client();
    let (resolved, _username) =
        resolve_and_refresh_gui_credentials(&pool, &http_client).await;

    assert!(
        resolved.is_none(),
        "sin refresco posible no puede descargarse: la descarga falla esta vez"
    );

    let (inv, reason) = account_flags(&pool, account_id).await;
    assert_eq!(
        inv, 0,
        "S185 H4: un error de transporte NO debe marcar credentials_invalid"
    );
    assert!(
        reason.is_none(),
        "S185 H4: invalid_reason debe permanecer NULL tras fallo de transporte"
    );
}

#[tokio::test]
async fn test_s185_expired_token_without_refresh_marks_invalid() {
    let pool = setup_test_db().await;

    // Fin de línea GENUINO: token vencido y sin refresh_token. Aquí sí procede
    // la marcación persistente (requiere re-login obligatoriamente).
    let creds = serde_json::json!({
        "access_token": "dead_access_token",
        "token_expiry": now_secs() - 1000.0,
        "expires_at": now_secs() - 1000.0,
    });
    let account_id =
        insert_tidal_account(&pool, "dead@test.example", &creds.to_string(), false, None).await;

    let http_client = reqwest::Client::new(); // no se usa: rama sin red
    let (resolved, _username) =
        resolve_and_refresh_gui_credentials(&pool, &http_client).await;
    assert!(resolved.is_none());

    let (inv, reason) = account_flags(&pool, account_id).await;
    assert_eq!(inv, 1, "expirado sin refresh_token sí se marca inválido");
    assert_eq!(
        reason.as_deref(),
        Some("token_expired"),
        "reason documentada para el UI"
    );
}

#[tokio::test]
async fn test_s185_login_upsert_cleans_stale_rows_pipeline_selects_clean_row() {
    let pool = setup_test_db().await;

    // Estado previo al re-login: filas Tidal envenenadas (como la fila real id=2).
    let stale = serde_json::json!({
        "access_token": "old_access",
        "refresh_token": "old_refresh",
        "token_expiry": now_secs() - 5000.0,
    });
    insert_tidal_account(&pool, "stale1@test.example", &stale.to_string(), true, Some("token_expired")).await;
    insert_tidal_account(&pool, "stale2@test.example", &stale.to_string(), true, Some("token_expired")).await;

    // «Login exitoso»: mismo camino que start_auth_and_save — payload con expiración
    // inyectada, cifrado y upsert estable por servicio.
    let mut payload = serde_json::json!({
        "access_token": "brand_new_access",
        "refresh_token": "brand_new_refresh",
        "user_id": "196616447",
        "country_code": "US",
    });
    inject_tidal_expiry(&mut payload, Some(now_secs() + 7200.0));
    let encrypted = crypto::encrypt(&payload.to_string()).unwrap();

    upsert_service_account(&pool, tidal_service_id(&pool).await, "Owner", Some("owner@test.example"), &encrypted)
        .await
        .expect("upsert succeeds");

    // Exactamente las mismas filas (preserva CASCADE data), TODAS limpias y
    // exactamente UNA activa — la que el pipeline selecciona.
    let rows: Vec<(i64, i64, Option<String>, i64)> = sqlx::query_as(
        "SELECT a.id, COALESCE(a.credentials_invalid, 0), a.invalid_reason, a.is_active FROM accounts a JOIN services s ON s.id=a.service_id WHERE s.name='tidal'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2, "el upsert preserva filas existentes");
    let active_count = rows.iter().filter(|(_, _, _, a)| *a == 1).count();
    assert_eq!(
        active_count, 1,
        "S185: tras el login debe quedar exactamente UNA fila activa"
    );
    for (_id, inv, reason, _active) in &rows {
        assert_eq!(*inv, 0, "todo flag obsoleto queda limpio tras el login");
        assert!(reason.is_none());
    }

    // El pipeline selecciona una fila limpia utilizable sin pasos extra.
    let http_client = reqwest::Client::new();
    let (resolved, _username) =
        resolve_and_refresh_gui_credentials(&pool, &http_client).await;
    assert!(
        resolved.is_some(),
        "tras un login exito la primera descarga NO requiere red ni pasos extra"
    );
    assert_eq!(resolved.unwrap().access_token, "brand_new_access");

    // El upsert sobre servicio sin filas inserta limpia y activa.
    sqlx::query("DELETE FROM accounts WHERE service_id = ?")
        .bind(tidal_service_id(&pool).await)
        .execute(&pool)
        .await
        .unwrap();
    upsert_service_account(&pool, tidal_service_id(&pool).await, "Fresh", None, &encrypted)
        .await
        .expect("insert path succeeds");
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM accounts WHERE service_id = ? AND is_active = 1 AND COALESCE(credentials_invalid,0)=0",
    )
    .bind(tidal_service_id(&pool).await)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "INSERT deja exactamente UNA fila activa limpia");
}

#[tokio::test]
async fn test_s185_relogin_with_different_email_survives_unique_constraint() {
    let pool = setup_test_db().await;

    // Dos filas históricas del mismo servicio (cuentas A y B del propietario).
    // El upsert antiguo moría aquí con «UNIQUE constraint failed: accounts.service_id,
    // accounts.email» al intentar el UPDATE masivo — fallando el login completo.
    insert_tidal_account(&pool, "account_a@test.example", r#"{"access_token":"a_old"}"#, false, None).await;
    let row_b = insert_tidal_account(&pool, "account_b@test.example", r#"{"access_token":"b_old"}"#, true, Some("token_expired")).await;

    let fresh = crypto::encrypt(r#"{"access_token":"relogin_access","refresh_token":"relogin_refresh","token_expiry":4102444800}"#).unwrap();
    let service_id = tidal_service_id(&pool).await;

    // 1) Re-login con la cuenta B: debe revivir SU fila sin error SQL.
    upsert_service_account(&pool, service_id, "Owner B", Some("account_b@test.example"), &fresh)
        .await
        .expect("re-login con email existente no debe chocar con UNIQUE");
    let (inv, reason, active) = sqlx::query_as::<_, (i64, Option<String>, i64)>(
        "SELECT COALESCE(credentials_invalid,0), invalid_reason, is_active FROM accounts WHERE id = ?",
    )
    .bind(row_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((inv, active), (0, 1));
    assert!(reason.is_none());

    // 2) Re-login con un email NUEVO: toma la fila más reciente, cambia su email,
    //    activa solo esa; ninguna violación de unicidad.
    upsert_service_account(&pool, service_id, "Owner C", Some("brand_new@test.example"), &fresh)
        .await
        .expect("re-login con email nuevo no debe chocar con UNIQUE");

    let actives: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounts WHERE service_id = ? AND is_active = 1",
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actives, 1);

    let (email_now,): (Option<String>,) = sqlx::query_as(
        "SELECT email FROM accounts WHERE service_id = ? AND is_active = 1",
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(email_now.as_deref(), Some("brand_new@test.example"));
}

#[tokio::test]
async fn test_s185_payload_without_expiry_gets_cached_or_conservative_expiry() {
    // 1. Cache disponible y futura → se usa la expiración real del device-flow.
    let mut payload = serde_json::json!({ "access_token": "a", "refresh_token": "r" });
    inject_tidal_expiry(&mut payload, Some(now_secs() + 86_400.0));
    assert!(payload["token_expiry"].as_f64().unwrap() > now_secs() + 80_000.0);
    assert_eq!(payload["expires_at"], payload["token_expiry"]);
    assert!(payload["expires_in"].as_f64().unwrap() > 80_000.0);

    // 2. Sin cache → ventana conservadora (usable offline, nunca eterna).
    //    Actualizado: auth.rs elevó el fallback de 1h a 4h (FIX 2026-08-25,
    //    CONSERVATIVE_TIDAL_EXPIRY_SECS = 14_400) porque 1h fabricaba
    //    expiraciones falsas al no poder leerse el caché de Python; esta
    //    expectativa quedó desalineada en esa sesión.
    let mut payload = serde_json::json!({ "access_token": "a", "refresh_token": "r" });
    inject_tidal_expiry(&mut payload, None);
    let expiry = payload["token_expiry"].as_f64().unwrap();
    assert!(expiry > now_secs() + 14_300.0 && expiry <= now_secs() + 14_400.0);

    // 3. Cache vencida (login viejo) → también cae al fallback conservador.
    let mut payload = serde_json::json!({ "access_token": "a", "refresh_token": "r" });
    inject_tidal_expiry(&mut payload, Some(now_secs() - 42.0));
    assert!(payload["token_expiry"].as_f64().unwrap() > now_secs());

    // 4. Payload que YA trae expiración → se preserva intacto.
    let mut payload = serde_json::json!({ "access_token": "a", "token_expiry": 123.5 });
    inject_tidal_expiry(&mut payload, Some(now_secs() + 99.0));
    assert_eq!(payload["token_expiry"].as_f64(), Some(123.5));
    assert!(payload.get("expires_in").is_none(), "no añade campos si ya hay expiración");
}

#[tokio::test]
async fn test_s185_worker_classification_ignores_generic_requires_auth_message() {
    // Mensaje genérico del pipeline: puede originarse por red transitoria → NO invalida.
    assert!(!classify_session_auth_failure(
        "RequiresAuth: No active or valid Tidal account credentials available. Please connect or re-authenticate Tidal in Settings > Accounts."
    ));
    // Error de transporte puro (el literal del log 259807) → NO invalida.
    assert!(!classify_session_auth_failure(
        "Network error on tidal [oauth2_token_refresh]: error sending request for url (https://auth.tidal.com/v1/oauth2/token)"
    ));

    // Evidencia específica de sesión muerta → SÍ invalida.
    assert!(classify_session_auth_failure(
        "OAuth token refresh failed: User token has expired and refresh failed"
    ));
    assert!(classify_session_auth_failure("Tidal returned invalid_grant for refresh"));
    assert!(classify_session_auth_failure("HTTP 401 during oauth token exchange"));
}
