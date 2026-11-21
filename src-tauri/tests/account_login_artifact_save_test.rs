//! Login en webview por cuenta (issue #84): el artefacto capturado entra al
//! CAMINO EXISTENTE de guardado (`save_auth_result`).
//!
//! Los tests usan ARTEFACTO FALSO (sin red, sin webview real — criterio 6 de
//! #84): el payload que el webview entregaría es idéntico en forma al que el
//! puente produce, y aquí se fija que el camino de guardado lo persiste igual
//! que un login del puente, SIN duplicar cuentas y SIN guardar ni registrar
//! contraseña ni correo del formulario.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::auth::save_auth_result;

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = syncify_tauri_lib::crypto::init_crypto([42u8; 32]);

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

async fn account_rows(
    pool: &sqlx::SqlitePool,
    service: &str,
) -> Vec<(i64, String, Option<String>)> {
    sqlx::query_as(
        r#"SELECT accounts.id, accounts.display_name, accounts.email
           FROM accounts JOIN services ON accounts.service_id = services.id
           WHERE services.name = ? ORDER BY accounts.id"#,
    )
    .bind(service)
    .fetch_all(pool)
    .await
    .expect("account rows")
}

async fn decrypted_credentials(pool: &sqlx::SqlitePool, service: &str) -> String {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("service id");
    let encrypted: String =
        sqlx::query_scalar("SELECT credentials_json FROM accounts WHERE service_id = ?")
            .bind(service_id)
            .fetch_one(pool)
            .await
            .expect("credentials");
    syncify_tauri_lib::crypto::decrypt(&encrypted).expect("decrypt credentials")
}

#[tokio::test]
async fn artefacto_falso_deezer_guarda_una_cuenta_por_el_camino_existente() {
    let pool = setup_test_db().await;

    // El payload que `artifact_payload("deezer", ...)` produce para un ARL
    // falso capturado en el webview.
    let arl = "x".repeat(150);
    let payload = serde_json::json!({
        "message": "Connected to Deezer",
        "arl": arl,
        "access_token": arl,
    });

    let result = save_auth_result("deezer", payload, &pool)
        .await
        .expect("el camino existente guarda el artefacto del webview");
    assert!(result.success);

    let rows = account_rows(&pool, "deezer").await;
    assert_eq!(rows.len(), 1, "una cuenta creada por el intento");

    let creds = decrypted_credentials(&pool, "deezer").await;
    assert!(
        creds.contains(&format!("\"arl\":\"{arl}\"")),
        "el ARL viaja cifrado"
    );

    // Re-login con el mismo artefacto NO duplica la cuenta (ADR-0007, decisión 6).
    let payload = serde_json::json!({ "arl": arl, "access_token": arl });
    save_auth_result("deezer", payload, &pool)
        .await
        .expect("re-login");
    let rows = account_rows(&pool, "deezer").await;
    assert_eq!(rows.len(), 1, "el re-login reutiliza la fila");
}

#[tokio::test]
async fn artefacto_falso_qobuz_guarda_solo_el_token_sin_correo_ni_contrasena() {
    let pool = setup_test_db().await;

    // El webview captura SOLO el token (criterio 2): sin username ni password,
    // el camino Qobuz no tiene fallback de credenciales que mezclar.
    let token = "q".repeat(30);
    let payload = serde_json::json!({
        "user_auth_token": token,
        "auth_token": token,
    });

    let result = save_auth_result("qobuz", payload, &pool)
        .await
        .expect("token viable persistido");
    assert!(result.success);

    let creds = decrypted_credentials(&pool, "qobuz").await;
    assert!(creds.contains(&token));
    assert!(
        !creds.contains("password") && !creds.contains("username") && !creds.contains("email"),
        "un login por artefacto no inventa credenciales de formulario: {creds}"
    );

    let rows = account_rows(&pool, "qobuz").await;
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].2.is_none(),
        "sin email capturado, la fila no lleva email"
    );
}

#[tokio::test]
async fn artefacto_de_tidal_del_puente_lleva_expiry_inyectado() {
    let pool = setup_test_db().await;

    // Payload de `handle_tidal("login_poll")` sin token_expiry: el camino
    // existente garantiza expiración (piso conservador) para el primer uso.
    let payload = serde_json::json!({
        "message": "Connected to Tidal",
        "access_token": "fake-tidal-access",
        "refresh_token": "fake-tidal-refresh",
        "user_id": "424242",
    });

    let result = save_auth_result("tidal", payload, &pool)
        .await
        .expect("saved");
    assert!(result.success);

    let creds = decrypted_credentials(&pool, "tidal").await;
    let value: serde_json::Value = serde_json::from_str(&creds).expect("json creds");
    assert!(
        value["token_expiry"].as_f64().is_some_and(|e| e > 0.0),
        "inject_tidal_expiry deja expiración: {creds}"
    );
    assert_eq!(value["refresh_token"], "fake-tidal-refresh");
}

#[tokio::test]
async fn artefacto_falso_soundcloud_y_apple_music_persisten() {
    let pool = setup_test_db().await;

    let sc = serde_json::json!({
        "oauth_token": format!("OAuth {}", "c".repeat(30)),
        "access_token": "c".repeat(30),
    });
    let result = save_auth_result("soundcloud", sc, &pool)
        .await
        .expect("saved");
    assert!(result.success);
    assert_eq!(account_rows(&pool, "soundcloud").await.len(), 1);

    let apple = serde_json::json!({
        "message": "Connected to Apple Music",
        "music_user_token": "m".repeat(25),
        "developer_token": format!("eyJ{}", "d".repeat(60)),
    });
    let result = save_auth_result("apple_music", apple, &pool)
        .await
        .expect("saved");
    assert!(result.success);
    assert_eq!(account_rows(&pool, "apple_music").await.len(), 1);

    let creds = decrypted_credentials(&pool, "apple_music").await;
    assert!(creds.contains("music_user_token"));
    assert!(!creds.contains("password"));
}
