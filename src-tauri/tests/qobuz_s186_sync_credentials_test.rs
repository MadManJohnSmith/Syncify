//! Sprint S186 «Qobuz auth hotfix» — pruebas de regresión.
//!
//! Cadena demostrada con evidencia de BD/logs del propietario
//! (syncify-dev.log:282786/:283332, accounts row id=5):
//!   el login por navegador captura sesión SIN token de API
//!   («didn't yield token from XHR headers», «JS fetch error»)
//!   → el puente devuelve success con user_auth_token=null + usuario/contraseña
//!   → start_auth_and_save guarda la fila activa SIN token
//!   → perform_get_service_auth_status trata user/pass como connected_valid
//!   → el dispatch de sync exigía token y fallaba SIEMPRE con
//!     «RequiresAuth: Qobuz user auth token missing in credentials»
//!   → la UI etiqueta todo requires_auth como «Token expirado».
//!
//! El pipeline de DESCARGA ya auto-logueaba con usuario/contraseña
//! (download/qobuz.rs); sync no. Estos tests fijan el contrato corregido:
//! 1. Token almacenado y viable se usa tal cual, sin red y sin reescritura.
//! 2. Sin token pero con usuario/contraseña: auto-login y PERSISTENCIA del
//!    token fresco en ESA fila (mismo contrato de escritor único que S185).
//! 3. Fallo de auto-login ⇒ RequiresAuth explícito, nunca el genérico «missing».
//! 4. Multi-fila: el refresco toca SOLO la fila objetivo; hermanas intactas.
//! 5. Email-distinto: upsert deja exactamente una fila activa utilizable.
//! 6. Los artefactos de consola pegados como contraseña se rechazan.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    is_plausible_qobuz_credential_value, resolve_qobuz_user_auth_token_with,
    upsert_service_account,
};
use syncify_tauri_lib::crypto;

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([186u8; 32]);

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

async fn qobuz_service_id(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("qobuz service seeded by migrations")
}

/// Fila Qobuz con credenciales cifradas; reproduce el payload forense real
/// (claves presentes con null incluido) cuando `credentials_json` lo pide.
async fn insert_qobuz_account(
    pool: &sqlx::SqlitePool,
    email: Option<&str>,
    credentials_json: &str,
    credentials_invalid: bool,
) -> i64 {
    let encrypted = crypto::encrypt(credentials_json).expect("encrypt succeeds");
    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json,
                                 credentials_invalid, invalid_reason, last_auth_error, is_active)
           VALUES (?, 'S186 Tester', ?, ?, ?, NULL, NULL, 1)
           RETURNING id"#,
    )
    .bind(qobuz_service_id(pool).await)
    .bind(email)
    .bind(&encrypted)
    .bind(credentials_invalid as i64)
    .fetch_one(pool)
    .await
    .expect("account insert succeeds")
}

async fn stored_credentials_json(pool: &sqlx::SqlitePool, account_id: i64) -> String {
    let encrypted: String =
        sqlx::query_scalar("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(pool)
            .await
            .expect("account row exists");
    crypto::decrypt(&encrypted).expect("decrypt succeeds")
}

fn ok_login(token: &'static str) -> impl Fn(String, String) -> std::future::Ready<Result<String, String>> {
    move |_u, _p| std::future::ready(Ok(token.to_string()))
}

/// Payload que replica EXACTAMENTE la fila envenenada del propietario:
/// tokens null + usuario real + "contraseña" que es un error de consola.
fn poisoned_owner_payload() -> serde_json::Value {
    serde_json::json!({
        "user_id": "rB8kTGqMtbqn8wfcvfcfAg==",
        "user_auth_token": null,
        "auth_token": null,
        "display_name": "rB8kTGqMtbqn8wfcvfcfAg==",
        "username": "owner@example.com",
        "password": "[Error] [Tauri] Command \"sync_service\" failed"
    })
}

#[tokio::test]
async fn test_s186_sync_uses_stored_viable_token_without_network_or_rewrite() {
    let pool = setup_test_db().await;

    let creds = serde_json::json!({
        "user_auth_token": "stored_viable_qobuz_token_123456",
        "username": "owner@example.com",
        "password": "real_password",
    });
    let account_id = insert_qobuz_account(&pool, Some("owner@example.com"), &creds.to_string(), false).await;
    let before = stored_credentials_json(&pool, account_id).await;

    let calls = Arc::new(AtomicUsize::new(0));
    let calls_clone = calls.clone();
    let resolved = resolve_qobuz_user_auth_token_with(
        &pool,
        account_id,
        &creds,
        move |_u, _p| {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Ok("SHOULD_NOT_BE_USED_1234567890".to_string()))
        },
    )
    .await
    .expect("stored viable token resolves");

    assert_eq!(resolved, "stored_viable_qobuz_token_123456");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "login must not run when a viable token is stored");
    assert_eq!(
        stored_credentials_json(&pool, account_id).await,
        before,
        "credentials_json must stay untouched when the stored token is usable"
    );
}

#[tokio::test]
async fn test_s186_sync_autologin_with_stored_credentials_persists_fresh_token() {
    let pool = setup_test_db().await;

    // Payload forense: tokens null, usuario/contraseña presentes (reales esta vez).
    let creds = poisoned_owner_payload();
    let creds = serde_json::json!({
        "user_id": creds["user_id"],
        "user_auth_token": null,
        "auth_token": null,
        "display_name": creds["display_name"],
        "username": "owner@example.com",
        "password": "RealCorrectPassword42",
    });
    let account_id = insert_qobuz_account(&pool, None, &creds.to_string(), false).await;

    let fresh = resolve_qobuz_user_auth_token_with(
        &pool,
        account_id,
        &creds,
        ok_login("fresh_auto_login_token_abcd1234"),
    )
    .await
    .expect("auto-login fallback resolves a fresh token");

    assert_eq!(fresh, "fresh_auto_login_token_abcd1234");

    let persisted: serde_json::Value =
        serde_json::from_str(&stored_credentials_json(&pool, account_id).await)
            .expect("persisted credentials stay valid JSON");
    assert_eq!(
        persisted["user_auth_token"].as_str(),
        Some("fresh_auto_login_token_abcd1234"),
        "fresh token must be persisted for later syncs/downloads"
    );
    assert_eq!(persisted["auth_token"], persisted["user_auth_token"]);
    assert_eq!(
        persisted["username"].as_str(),
        Some("owner@example.com"),
        "pre-existing fields must be preserved by the whole-payload replace"
    );
    assert_eq!(
        persisted["display_name"].as_str(),
        Some("rB8kTGqMtbqn8wfcvfcfAg=="),
        "non-credential metadata survives the refresh"
    );

    let flags: i64 = sqlx::query_scalar("SELECT COALESCE(credentials_invalid, 0) FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .expect("row exists");
    assert_eq!(flags, 0, "refreshed credentials are clean");
}

#[tokio::test]
async fn test_s186_sync_autologin_failure_reports_requires_auth_not_missing() {
    let pool = setup_test_db().await;

    let creds = serde_json::json!({
        "user_auth_token": null,
        "username": "owner@example.com",
        "password": "wrong_or_poisoned_password",
    });
    let account_id = insert_qobuz_account(&pool, None, &creds.to_string(), false).await;

    let err = resolve_qobuz_user_auth_token_with(
        &pool,
        account_id,
        &creds,
        |_u, _p| std::future::ready(Err("Qobuz API error (401): bad credentials".to_string())),
    )
    .await
    .expect_err("failed auto-login surfaces an error");

    assert!(
        err.starts_with("RequiresAuth: Qobuz auto-login with stored credentials failed"),
        "error must name the auto-login failure explicitly, got: {err}"
    );
    assert!(
        !err.contains("missing in credentials"),
        "the generic pre-S186 'missing' message must not reappear for this state"
    );
}

#[tokio::test]
async fn test_s186_multiline_refresh_touches_only_target_row() {
    let pool = setup_test_db().await;

    // Hermana vieja: inválida e inactiva (resto de ciclos previos).
    let old_creds = serde_json::json!({ "user_auth_token": null, "username": "old@example.com", "password": "old_pw" });
    let old_id = insert_qobuz_account(&pool, Some("old@example.com"), &old_creds.to_string(), true).await;
    sqlx::query("UPDATE accounts SET is_active = 0, invalid_reason = 'HTTP 401' WHERE id = ?")
        .bind(old_id)
        .execute(&pool)
        .await
        .expect("deactivate sibling");

    // Fila objetivo activa sin token.
    let target_creds = serde_json::json!({ "user_auth_token": null, "username": "new@example.com", "password": "new_pw" });
    let target_id = insert_qobuz_account(&pool, Some("new@example.com"), &target_creds.to_string(), false).await;

    resolve_qobuz_user_auth_token_with(&pool, target_id, &target_creds, ok_login("multiline_fresh_token_12345"))
        .await
        .expect("target row resolves");

    let target_persisted = stored_credentials_json(&pool, target_id).await;
    assert!(
        target_persisted.contains("multiline_fresh_token_12345"),
        "target row receives the fresh token"
    );

    let (old_json, old_flags, old_reason, old_active): (String, i64, Option<String>, i64) =
        sqlx::query_as(
            "SELECT credentials_json, COALESCE(credentials_invalid,0), invalid_reason, is_active FROM accounts WHERE id = ?",
        )
        .bind(old_id)
        .fetch_one(&pool)
        .await
        .expect("sibling row exists");

    assert_eq!(old_flags, 1, "sibling stays flagged exactly as before");
    assert_eq!(old_reason.as_deref(), Some("HTTP 401"));
    assert_eq!(old_active, 0, "sibling stays inactive");
    assert_eq!(
        crypto::decrypt(&old_json).unwrap(),
        old_creds.to_string(),
        "sibling credentials_json is byte-for-byte untouched"
    );
}

#[tokio::test]
async fn test_s186_email_distinct_login_leaves_single_usable_active_row() {
    let pool = setup_test_db().await;
    let service_id = qobuz_service_id(&pool).await;

    let encrypted_a = crypto::encrypt(r#"{"user_auth_token":"stale_old_row_token_999999"}"#).unwrap();
    sqlx::query(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, credentials_invalid, is_active)
           VALUES (?, 'Old', 'a@example.com', ?, 1, 1)"#,
    )
    .bind(service_id)
    .bind(&encrypted_a)
    .execute(&pool)
    .await
    .expect("insert row A");

    let encrypted_b = crypto::encrypt(r#"{"user_auth_token":null,"username":"b@example.com","password":"pw_b"}"#).unwrap();
    sqlx::query(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, credentials_invalid, is_active)
           VALUES (?, 'New', 'b@example.com', ?, 0, 1)"#,
    )
    .bind(service_id)
    .bind(&encrypted_b)
    .execute(&pool)
    .await
    .expect("insert row B");

    // Re-login con el email B: debe activar/limpiar B y desactivar A con flags limpios.
    upsert_service_account(
        &pool,
        service_id,
        "New Login",
        Some("b@example.com"),
        &crypto::encrypt(r#"{"user_auth_token":"brand_new_login_token_424242"}"#).unwrap(),
    )
    .await
    .expect("upsert succeeds");

    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        r#"SELECT IFNULL(email,''), is_active, credentials_json FROM accounts a
           JOIN services s ON s.id = a.service_id
           WHERE s.name = 'qobuz' ORDER BY a.id"#,
    )
    .fetch_all(&pool)
    .await
    .expect("rows exist");

    assert_eq!(rows.len(), 2);
    let active_rows: Vec<&(String, i64, String)> = rows.iter().filter(|(_, active, _)| *active == 1).collect();
    assert_eq!(
        active_rows.len(),
        1,
        "exactly one usable active row after reconnect, got: {rows:?}"
    );
    assert_eq!(active_rows[0].0, "b@example.com");
    assert!(
        crypto::decrypt(&active_rows[0].2).unwrap().contains("brand_new_login_token_424242"),
        "active row carries the fresh login credentials"
    );

    // La fila seleccionable por sync (ORDER BY id DESC LIMIT 1 WHERE is_active=1) resuelve.
    let (sel_id, sel_json): (i64, String) = sqlx::query_as(
        r#"SELECT a.id, a.credentials_json FROM accounts a
           JOIN services s ON s.id = a.service_id
           WHERE s.name = 'qobuz' AND a.is_active = 1
           ORDER BY a.id DESC LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .expect("sync-selectable row exists");

    let sel_creds: serde_json::Value =
        serde_json::from_str(&crypto::decrypt(&sel_json).unwrap()).expect("valid JSON");
    let resolved = resolve_qobuz_user_auth_token_with(&pool, sel_id, &sel_creds, |_, _| {
        std::future::ready(Err("must_not_be_called".to_string()))
    })
    .await
    .expect("freshly connected row is instantly usable by sync");
    assert_eq!(resolved, "brand_new_login_token_424242");
}

#[tokio::test]
async fn test_s186_console_artifact_password_never_qualifies() {
    let pool = setup_test_db().await;

    let artifact = "[Error] [Tauri] Command \"sync_service\" failed: – \"RequiresAuth: Qobuz user auth token missing in credentials\"";
    assert!(!is_plausible_qobuz_credential_value(artifact));
    assert!(!is_plausible_qobuz_credential_value("[Errno 11] whatever"));
    assert!(!is_plausible_qobuz_credential_value("login failed: boom"));
    assert!(!is_plausible_qobuz_credential_value(""));
    assert!(!is_plausible_qobuz_credential_value(&"x".repeat(200)));

    assert!(is_plausible_qobuz_credential_value("S3cure-Passw0rd!"));
    assert!(is_plausible_qobuz_credential_value("plainpassword"));
    assert!(is_plausible_qobuz_credential_value("alansalasctf@gmail.com"));

    // La fila envenenada real NO puede auto-loguearse: el resolver la rechaza
    // en seco (sin round-trip) con un mensaje accionable.
    let poisoned = poisoned_owner_payload();
    let account_id = insert_qobuz_account(&pool, None, &poisoned.to_string(), false).await;
    let err = resolve_qobuz_user_auth_token_with(
        &pool,
        account_id,
        &poisoned,
        |_, _| std::future::ready(Ok("token_from_bad_password_12345".to_string())),
    )
    .await
    .expect_err("poisoned password cannot produce a working account silently");

    assert!(
        err.contains("no usable username/password fallback"),
        "implausible credentials must be rejected before any network attempt, got: {err}"
    );
}
