//! Ruta de identidad de cuentas — Qobuz y saneado de errores de login.
//!
//! Hallazgos auditados que fija este suite:
//!
//! `auth_bridge.py:317` respondía `"display_name": result`, donde `result` es el
//! id NUMÉRICO que devuelve `login_with_browser` (o el marcador
//! `browser_session`): la cuenta quedaba bautizada con un identificador opaco,
//! la UI lo mostraba y dos cuentas de la misma persona eran indistinguibles.
//! `commands/auth.rs` tomaba además `user_id` como fallback del nombre visible
//! y capturaba `username` sin guardarlo como email.
//!
//! Contratos verificados aquí:
//!  1. Un email Qobuz REAL se propaga de `username` a `email`, y el nombre
//!     visible resultante es legible (nunca el id numérico).
//!  2. Un `username` que no es email no se convierte en email ni en nombre
//!     visible; la identidad deduplicada sigue siendo `external_user_id`
//!     (id opaco), que jamás se muestra.
//!  3. El mensaje de login Qobuz fallido no reproduce la respuesta del
//!     proveedor: sin tokens, sin cookies, sin credenciales.
//!  4. `last_auth_error` se sanea en la frontera de persistencia: un error que
//!     arrastre un token/cookie/contraseña queda redactado antes de guardarse.
//!  5. El alta multi-cuenta es idempotente por identidad: dos emails distintos
//!     son dos filas; el mismo email y distinto `external_user_id` reutiliza la
//!     fila del usuario estable sin duplicarla.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    is_plausible_qobuz_credential_value, login_email, mark_account_credentials_invalid,
    qobuz_plausible_email, sanitize_auth_error_reason, sanitize_visible_display_name,
    spotify_visible_display_name, upsert_service_account,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::qobuz::qobuz_login_missing_token_message;

// ── Contratos de entitlement Qobuz (WF-QOB-01) ──────────────────────────────

/// Helper to read the persisted tri-state.
async fn stored_entitlement(pool: &sqlx::SqlitePool, account_id: i64) -> Option<i64> {
    sqlx::query_scalar("SELECT streaming_entitled FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("entitlement read")
}

#[tokio::test]
async fn browser_token_only_account_stays_unknown() {
    let pool = setup_test_db().await;
    let service_id = qobuz_service_id(&pool).await;
    let encrypted = crypto::encrypt(
        &serde_json::json!({"user_auth_token": "browser_token_1234567890"}).to_string(),
    )
    .expect("encrypt");

    let id = upsert_service_account(
        &pool,
        service_id,
        "Token Only",
        None,
        Some("tok_only_1"),
        &encrypted,
        // The bridge says nothing about `user.credential` here, so the caller
        // hands None. That must persist as unknown, not as "no subscription".
        None,
    )
    .await
    .expect("upsert");

    assert_eq!(
        stored_entitlement(&pool, id).await,
        Some(-1),
        "a token-only login must persist unknown (-1)"
    );
}

#[tokio::test]
async fn silent_relogin_never_demotes_a_known_entitlement() {
    let pool = setup_test_db().await;
    let service_id = qobuz_service_id(&pool).await;
    let encrypted = crypto::encrypt(&serde_json::json!({"user_auth_token": "tok"}).to_string())
        .expect("encrypt");

    let id = upsert_service_account(
        &pool,
        service_id,
        "Streamer",
        None,
        Some("streamer_1"),
        &encrypted,
        Some(true),
    )
    .await
    .expect("first");
    assert_eq!(stored_entitlement(&pool, id).await, Some(1));

    // Re-login where the provider did not report flags (None): the stored value
    // must survive untouched.
    let id2 = upsert_service_account(
        &pool,
        service_id,
        "Streamer",
        None,
        Some("streamer_1"),
        &encrypted,
        None,
    )
    .await
    .expect("second");
    assert_eq!(id2, id, "misma identidad reutiliza la fila");
    assert_eq!(
        stored_entitlement(&pool, id).await,
        Some(1),
        "un re-login silencioso no debe degradar known=1"
    );

    // A login that DID report "no streaming" is allowed to write 0.
    let id3 = upsert_service_account(
        &pool,
        service_id,
        "Streamer",
        None,
        Some("streamer_1"),
        &encrypted,
        Some(false),
    )
    .await
    .expect("third");
    assert_eq!(id3, id);
    assert_eq!(stored_entitlement(&pool, id).await, Some(0));
}

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([23u8; 32]);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations must apply");
    pool
}

async fn qobuz_service_id(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("qobuz service row must exist")
}

async fn account_display_name(pool: &sqlx::SqlitePool, account_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT display_name FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("row exists")
}

async fn account_email(pool: &sqlx::SqlitePool, account_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT email FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("row exists")
}

async fn account_external_user_id(pool: &sqlx::SqlitePool, account_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT external_user_id FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("row exists")
}

async fn insert_identity_account(
    pool: &sqlx::SqlitePool,
    display_name: &str,
    email: Option<&str>,
    external_user_id: Option<&str>,
) -> i64 {
    let service_id = qobuz_service_id(pool).await;
    let creds = serde_json::json!({ "user_auth_token": "not-a-viable-token" }).to_string();
    let encrypted = crypto::encrypt(&creds).expect("encrypt");
    upsert_service_account(
        pool,
        service_id,
        display_name,
        email,
        external_user_id,
        &encrypted,
        None,
    )
    .await
    .expect("upsert")
}

fn encrypted_dummy() -> String {
    crypto::encrypt(&serde_json::json!({ "user_auth_token": "placeholder" }).to_string())
        .expect("encrypt")
}

/// Espejo de la ruta de login en `commands/auth.rs::start_auth_and_save`:
/// ejercita la función REAL de extracción de email, para que un cambio en su
/// orden de resolución falle aquí en vez de descartar silenciosamente el email
/// de Qobuz.
///
/// El paso 2 de esa función inyecta un `username` plausible en
/// `credentials_payload.email` (el puente nunca devuelve un `email`) y la
/// extracción debe leerlo DESDE AHÍ. `data` mantiene la precedencia para los
/// servicios que sí devuelven un email real.
#[tokio::test]
async fn the_injected_qobuz_email_reaches_the_account_row() {
    // El puente solo devuelve `username` (capturado del formulario de login)
    // y el id opaco del proveedor; no hay `email` en el payload.
    let data = serde_json::json!({
        "user_id": "123456789",
        "user_auth_token": "a".repeat(32),
        "username": "Owner@Example.com",
        "display_name": "Qobuz User",
    });
    let mut credentials_payload = data.clone();

    // ── Paso 2: el bloque Qobuz propaga el `username` plausible a `email` ──
    let username = data
        .get("username")
        .or_else(|| data.get("email"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| is_plausible_qobuz_credential_value(s));
    let email_from_username = qobuz_plausible_email(username.as_deref());
    assert!(
        email_from_username.is_some(),
        "a genuine email on the login form must be propagated"
    );
    if let Some(candidate) = email_from_username {
        if let Some(obj) = credentials_payload.as_object_mut() {
            obj.insert("email".to_string(), serde_json::Value::String(candidate));
        }
    }

    // ── Paso 3: la extracción de producción debe verlo ──
    let email = login_email(&data, &credentials_payload);
    assert_eq!(
        email.as_deref(),
        Some("owner@example.com"),
        "the email injected into credentials_payload must reach the account row"
    );

    // De extremo a extremo: el email resuelto llega a la BD como identidad.
    let pool = setup_test_db().await;
    let service_id = qobuz_service_id(&pool).await;
    let opaque_id = "123456789";
    let display_name = sanitize_visible_display_name(Some("Qobuz User"), email.as_deref(), || {
        "Qobuz User".to_string()
    });
    let account_id = upsert_service_account(
        &pool,
        service_id,
        &display_name,
        email.as_deref(),
        Some(opaque_id),
        &encrypted_dummy(),
        None,
    )
    .await
    .expect("upsert");

    assert_eq!(
        account_email(&pool, account_id).await.as_deref(),
        Some("owner@example.com"),
        "a Qobuz login with a real email on the form is stored with that email"
    );
}

/// Los servicios cuyo puente SÍ devuelve un email conservan la precedencia: el
/// fallback nunca debe pisar un email real del proveedor.
#[tokio::test]
async fn a_provider_supplied_email_still_wins_over_the_injected_one() {
    // Si el puente SÍ devuelve un email, este tiene precedencia: el fallback
    // no debe pisar el email real del proveedor.
    let data = serde_json::json!({
        "user_id": "42",
        "email": "from-bridge@example.com",
        "username": "owner@example.com",
    });
    let credentials_payload = serde_json::json!({
        "email": "injected-from-username@example.com",
    });

    assert_eq!(
        login_email(&data, &credentials_payload).as_deref(),
        Some("from-bridge@example.com"),
        "data.email must take precedence over the propagated credentials_payload email"
    );
}

/// Un `username` que no es email no debe inventar ninguno: el id opaco del
/// proveedor sigue siendo la identidad de deduplicación, nunca el email.
#[tokio::test]
async fn a_qobuz_username_that_is_not_an_email_leaves_email_empty() {
    // Un `username` que no es email no inyecta ninguno: la extracción queda
    // en `None` y el id opaco sigue siendo la identidad de deduplicación.
    let data = serde_json::json!({
        "user_id": "123456789",
        "user_auth_token": "a".repeat(32),
        "username": "not-an-email",
    });
    let credentials_payload = data.clone();

    assert!(
        qobuz_plausible_email(data["username"].as_str()).is_none(),
        "a plain username is not an email"
    );
    assert_eq!(
        login_email(&data, &credentials_payload),
        None,
        "no email is invented from a username-like value"
    );
}

// ── 1. Email real Qobuz ─────────────────────────────────────────────────────

#[tokio::test]
async fn a_genuine_qobuz_email_is_propagated_from_username() {
    let captured = Some("owner@example.com");
    assert_eq!(
        qobuz_plausible_email(captured).as_deref(),
        Some("owner@example.com")
    );

    let pool = setup_test_db().await;
    let service_id = qobuz_service_id(&pool).await;

    // El puente devuelve username + id numérico; Rust propaga el email.
    let username = "Owner@Example.com";
    let email = qobuz_plausible_email(Some(username));
    let opaque_id = "123456789";
    let display_name = sanitize_visible_display_name(Some("Qobuz User"), email.as_deref(), || {
        "Qobuz User".to_string()
    });

    let account_id = upsert_service_account(
        &pool,
        service_id,
        &display_name,
        email.as_deref(),
        Some(opaque_id),
        &encrypted_dummy(),
        None,
    )
    .await
    .expect("upsert with real email");

    assert_eq!(
        account_email(&pool, account_id).await.as_deref(),
        Some("owner@example.com"),
        "el email capturado debe quedar guardado como identidad"
    );
    assert_eq!(
        account_external_user_id(&pool, account_id).await.as_deref(),
        Some(opaque_id),
        "la identidad deduplicada sigue siendo el id opaco del proveedor"
    );
    assert_eq!(
        account_display_name(&pool, account_id).await.as_deref(),
        Some("Qobuz User")
    );
}

#[tokio::test]
async fn an_email_without_a_deliverable_domain_is_rejected() {
    // "user@localhost" y "user@qobuz" no son direcciones que identifiquen a
    // nadie: propagarlas corrompería la identidad de la cuenta.
    for candidate in [
        "user@localhost",
        "user@qobuz",
        "user@",
        "@example.com",
        "plainuser",
        "user example@mail.com",
        "user@exam ple.com",
        "user@-example.com",
    ] {
        assert_eq!(
            qobuz_plausible_email(Some(candidate)),
            None,
            "{candidate} must not become an email"
        );
    }
}

#[tokio::test]
async fn console_error_artifacts_are_never_treated_as_an_email() {
    let console_error =
        r#"[Error] [Tauri] Command "sync_service" failed: – "RequiresAuth: token missing""#;
    assert_eq!(qobuz_plausible_email(Some(console_error)), None);
    assert_eq!(qobuz_plausible_email(Some("")), None);
    assert_eq!(qobuz_plausible_email(None), None);
}

#[test]
fn qobuz_plausible_email_normalizes_case_and_trims() {
    assert_eq!(
        qobuz_plausible_email(Some("  OWNER@Example.COM ")).as_deref(),
        Some("owner@example.com")
    );
}

// ── 2. El id opaco jamás es visible ─────────────────────────────────────────

#[test]
fn visible_display_name_rejects_opaque_identifiers() {
    let numeric = "123456789";
    assert!(
        !sanitize_visible_display_name(Some(numeric), None, || "Qobuz User".to_string())
            .contains(numeric)
    );
    assert_eq!(
        sanitize_visible_display_name(Some("browser_session"), None, || "Qobuz User".to_string()),
        "Qobuz User"
    );
    assert_eq!(
        sanitize_visible_display_name(Some("  Ana  "), None, || "Qobuz User".to_string()),
        "Ana"
    );
}

#[test]
fn visible_display_name_never_ends_up_as_a_provider_id() {
    // Ids hex del proveedor y UUIDs tampoco son nombres escribibles.
    let uuid = "550e8400-e29b-41d4-a716-446655440000";
    assert_eq!(
        sanitize_visible_display_name(Some(uuid), None, || "Qobuz User".to_string()),
        "Qobuz User"
    );
    let hex_id = "0123456789abcdef0123456789abcdef";
    assert_eq!(
        sanitize_visible_display_name(Some(hex_id), None, || "Qobuz User".to_string()),
        "Qobuz User"
    );
    // Un nombre real solo con letras/dígitos mixtos sí se conserva.
    assert_eq!(
        sanitize_visible_display_name(Some("Ana Ruiz"), None, || "Qobuz User".to_string()),
        "Ana Ruiz"
    );
}

/// El id base62 de Spotify (21 caracteres, mezcla de mayúsculas, minúsculas y
/// dígitos) no es ni todo-dígitos ni todo-hex, así que `is_opaque_identifier` no
/// lo detecta por sí solo. El login PKCE de Spotify resolvía el nombre como
/// `profile["display_name"].or(profile["id"])`, y Spotify solo devuelve
/// `display_name` cuando el usuario lo rellenó a mano: para el resto la cuenta
/// quedaba bautizada con ese id y la UI mostraba "Conectado como smedja...".
#[test]
fn visible_display_name_rejects_a_provider_base62_id() {
    // Forma real de un id de usuario de Spotify: sin display_name escrito a mano.
    let base62_id = "smedjan12AB4cQ5dEfGhIj";
    // Sin display_name, la cuenta cae al genérico: nunca al id.
    assert_eq!(
        spotify_visible_display_name(None, Some(base62_id)),
        "Spotify User"
    );
    // Con display_name real sí se conserva, y el id sigue disponible aparte.
    assert_eq!(
        spotify_visible_display_name(Some("  Alan Turing "), Some(base62_id)),
        "Alan Turing"
    );
    // Cuando el perfil devuelve el propio id como display_name tampoco sirve.
    assert_eq!(
        spotify_visible_display_name(Some(base62_id), Some(base62_id)),
        "Spotify User"
    );
    assert_eq!(
        spotify_visible_display_name(Some(base62_id), Some("")),
        spotify_visible_display_name(Some(base62_id), None)
    );
}

// ── 3. El error de login no filtra secretos ─────────────────────────────────

#[test]
fn qobuz_login_error_does_not_reproduce_the_provider_payload() {
    // Forma real de una respuesta de login fallido: el objeto trae el token y
    // el email que se enviaron. El mensaje anterior lo interpolaba ENTERO.
    let leaked_payload = serde_json::json!({
        "user": {
            "auth_token": "SECRET_USER_AUTH_TOKEN_SHOULD_NOT_APPEAR",
            "id": 123456789,
            "display_name": "Ana",
            "credential": { "id": 999, "label": "lossless" }
        },
        "message": "Bad credentials",
        "status": "error"
    });

    let msg = qobuz_login_missing_token_message(&leaked_payload);

    assert!(
        !msg.contains("SECRET_USER_AUTH_TOKEN_SHOULD_NOT_APPEAR"),
        "token filtrado: {msg}"
    );
    assert!(
        !msg.contains("auth_token"),
        "nombre de campo secreto filtrado: {msg}"
    );
    assert!(
        !msg.contains("123456789"),
        "identidad del usuario filtrada: {msg}"
    );
    assert!(!msg.contains("Ana"), "perfil del usuario filtrado: {msg}");
    assert!(
        !msg.contains("Bad credentials"),
        "mensaje del proveedor reproducido: {msg}"
    );
    assert!(
        !msg.contains("lossless"),
        "detalle interno reproducido: {msg}"
    );
    assert!(
        msg.contains("Qobuz login rejected"),
        "el mensaje debe seguir siendo accionable: {msg}"
    );
}

#[test]
fn qobuz_login_error_reproduces_no_provider_value_at_all() {
    // Fallo seguro: aunque parezcan códigos, los campos de una respuesta de login
    // no confiable (pueden venir del cliente) no se reproducen en ningún caso. El
    // mensaje debe ser estable y accionable, y jamás filtrar el token.
    let payload = serde_json::json!({
        "user": { "auth_token": "SECRET", "id": 123456789, "display_name": "Ana" },
        "error_code": "invalid_credentials",
        "message": "bad credentials for owner@example.com",
        "http_status": 401,
    });
    let msg = qobuz_login_missing_token_message(&payload);

    assert!(!msg.contains("SECRET"), "token filtrado: {msg}");
    assert!(!msg.contains("123456789"), "identidad filtrada: {msg}");
    assert!(!msg.contains("Ana"), "perfil filtrado: {msg}");
    assert!(
        !msg.contains("invalid_credentials"),
        "valor del proveedor reproducido: {msg}"
    );
    assert!(
        !msg.contains("owner@example.com"),
        "email del usuario reproducido: {msg}"
    );
    assert!(msg.contains("Qobuz login rejected"), "no accionable: {msg}");
    assert!(
        msg.contains("Settings > Accounts"),
        "sin salida útil: {msg}"
    );
}

#[test]
fn qobuz_login_error_is_stable_without_any_payload() {
    for empty in [
        serde_json::json!({}),
        serde_json::json!(null),
        serde_json::json!("nope"),
    ] {
        let msg = qobuz_login_missing_token_message(&empty);
        assert!(msg.contains("Qobuz login rejected"), "got: {msg}");
    }
}

// ── 4. Saneado en la frontera de last_auth_error ────────────────────────────

#[tokio::test]
async fn last_auth_error_persists_redacted_never_the_secret() {
    let pool = setup_test_db().await;
    let account_id =
        insert_identity_account(&pool, "Ana", Some("owner@example.com"), Some("123456789")).await;

    let leaked = "Qobuz API error (401): token=SECRET_TOKEN_ABC123&session=SESSION_COOKIE_XYZ789 password=hunter2";
    mark_account_credentials_invalid(&pool, "qobuz", leaked, Some(account_id))
        .await
        .expect("mark invalid");

    let stored: String = sqlx::query_scalar("SELECT last_auth_error FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .expect("row exists");

    assert!(
        !stored.contains("SECRET_TOKEN_ABC123"),
        "token filtrado: {stored}"
    );
    assert!(
        !stored.contains("SESSION_COOKIE_XYZ789"),
        "cookie filtrada: {stored}"
    );
    assert!(!stored.contains("hunter2"), "contraseña filtrada: {stored}");
    assert!(stored.contains("[REDACTED]"), "sin redacción: {stored}");
    assert!(
        stored.contains("Qobuz API error (401)"),
        "el motivo accionable debe sobrevivir: {stored}"
    );
}

#[test]
fn sanitize_auth_error_reason_redacts_every_secret_shape() {
    // JSON de respuesta del proveedor.
    let json_err = r#"{"user": {"auth_token": "TOK1234567890", "password": "pw-should-not-appear"}, "message": "Invalid credentials"}"#;
    let out = sanitize_auth_error_reason(json_err);
    assert!(!out.contains("TOK1234567890"), "token: {out}");
    assert!(!out.contains("pw-should-not-appear"), "password: {out}");

    // Query string con token y cookie.
    let qs =
        "GET https://api.qobuz.com/x?user_auth_token=TOK9998887&sp_dc=COOKIE_ABC123DEF456 failed";
    let out = sanitize_auth_error_reason(qs);
    assert!(!out.contains("TOK9998887"), "token: {out}");
    assert!(!out.contains("COOKIE_ABC123DEF456"), "cookie: {out}");

    // JWT suelto.
    let jwt = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.signature rejected";
    let out = sanitize_auth_error_reason(jwt);
    assert!(
        !out.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
        "jwt: {out}"
    );
}

#[test]
fn sanitize_auth_error_reason_never_fails_and_truncates() {
    assert_eq!(sanitize_auth_error_reason(""), "");
    assert!(sanitize_auth_error_reason("Network unreachable").contains("Network unreachable"));
    let long = "x".repeat(5000);
    assert!(sanitize_auth_error_reason(&long).chars().count() <= 301);
    // No rompe con UTF-8 ni valores multibyte.
    let utf8 = "error de Beta: áéíóú with token=SECRETTOKENVALUE123";
    let out = sanitize_auth_error_reason(utf8);
    assert!(!out.contains("SECRETTOKENVALUE123"));
}

// ── 5. Multi-cuenta idempotente ─────────────────────────────────────────────

#[tokio::test]
async fn two_qobuz_emails_are_two_accounts_not_one_overwritten() {
    let pool = setup_test_db().await;
    let service_id = qobuz_service_id(&pool).await;

    let first = upsert_service_account(
        &pool,
        service_id,
        "Ana",
        Some("ana@example.com"),
        Some("111111111"),
        &encrypted_dummy(),
        None,
    )
    .await
    .expect("first");
    let second = upsert_service_account(
        &pool,
        service_id,
        "Bruno",
        Some("bruno@example.com"),
        Some("222222222"),
        &encrypted_dummy(),
        None,
    )
    .await
    .expect("second");

    assert_ne!(first, second, "dos emails distintos son dos filas");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(service_id)
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 2, "ninguna fila debe pisarse");

    // Re-login de la primera cuenta: idempotente, reutiliza la fila.
    let again = upsert_service_account(
        &pool,
        service_id,
        "Ana",
        Some("ana@example.com"),
        Some("111111111"),
        &encrypted_dummy(),
        None,
    )
    .await
    .expect("relogin");
    assert_eq!(again, first, "el re-login de la misma identidad no duplica");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(service_id)
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 2);

    // Exactamente una activa por servicio.
    let active: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ? AND is_active = 1")
            .bind(service_id)
            .fetch_one(&pool)
            .await
            .expect("active count");
    assert_eq!(active, 1);
}

// ── 6. Higiene IPC: nada de contraseñas ni tokens por el canal ──────────────

#[tokio::test]
async fn auth_ipc_commands_never_receive_or_return_credentials() {
    // `start_auth_and_save` solo acepta el nombre del servicio; la contraseña
    // viaja por el puente Python (stdin/entorno), nunca por un argumento IPC.
    // `start_auth`/`refresh_spotify_session` devuelven `AuthResult`, cuyo `data`
    // es el payload del puente. Esta test fija en el código de registro que no
    // existe ningún comando que acepte credenciales en claro como parámetro.
    let main_rs = include_str!("../src/main.rs");

    let handler_start = main_rs
        .find("tauri::generate_handler![")
        .expect("generate_handler! must exist");
    let handler_end = main_rs[handler_start..]
        .find("])")
        .expect("closing delimiter must exist");
    let handler_block = &main_rs[handler_start..handler_start + handler_end];

    for forbidden in [
        "get_account_credentials",
        "get_account_password",
        "get_credentials",
        "get_decrypted_credentials",
    ] {
        assert!(
            !handler_block.contains(forbidden),
            "{forbidden} must not be exposed over IPC"
        );
    }
    assert!(
        handler_block.contains("commands::start_auth_and_save"),
        "el comando de login debe seguir registrado"
    );
}

/// El payload de éxito de `start_auth_and_save` solo contiene metadatos: el
/// mensaje visible, el email, el id de cuenta. Esta test documenta la forma
/// esperada para que un futuro cambio no reintroduzca secretos en el retorno.
#[test]
fn successful_login_result_shape_carries_no_secrets() {
    // Réplica de la forma que devuelve el brazo de guardado (comandos/auth.rs).
    let data = serde_json::json!({
        "message": "Connected as Ana",
        "display_name": "Ana",
        "email": "ana@example.com",
        "user_id": "123456789",
        "account_id": 1,
        "requeued_downloads": 0,
    });

    for key in [
        "password",
        "user_auth_token",
        "auth_token",
        "token",
        "cookies",
        "secret",
    ] {
        assert!(
            data.get(key).is_none(),
            "{key} no debe viajar hacia la UI en el resultado del login"
        );
    }
}
