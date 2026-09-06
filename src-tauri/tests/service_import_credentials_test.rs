//! Tests for TASK-37: Dynamic credentials loading for Tidal in `import_service`
//!
//! Validates:
//! 1. `import_service("tidal")` loads credentials dynamically from SQLite `accounts` without SO env vars.
//! 2. Encrypted credentials (`crypto::encrypt`) are properly decrypted.
//! 3. Plaintext credentials work with transparent fallback.
//! 4. User ID can be extracted from JSON (string/number) or JWT payload.
//! 5. When no account exists, returns friendly error instead of panicking.
//! 6. Accounts marked `credentials_invalid = 1` return `RequiresAuth` error.
//! 7. Fallback to `TIDAL_ACCESS_TOKEN` only works if user ID is configured/extractable (never hardcoded mock).

use std::sync::{Arc, Mutex};
use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    extract_user_id_from_jwt, import_service, resolve_tidal_import_credentials,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::{AppState, EnrichmentWorkerState};
use tauri::Manager;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug)]
struct MockRequest {
    target: String,
    auth_header: Option<String>,
}

async fn spawn_mock_tidal(
    response_body: String,
) -> (String, Arc<Mutex<Vec<MockRequest>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::<MockRequest>::new()));
    let reqs = requests.clone();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { break };
            let reqs = reqs.clone();
            let body = response_body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);
                let target = raw
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("")
                    .to_string();
                let auth_header = raw
                    .lines()
                    .find(|l| l.to_lowercase().starts_with("authorization:"))
                    .map(|l| l.trim().to_string());

                reqs.lock().unwrap().push(MockRequest {
                    target,
                    auth_header,
                });

                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });

    (format!("http://{}", addr), requests)
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

fn create_test_app(pool: sqlx::SqlitePool) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);
    app
}

fn make_test_jwt(sub: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"sub":"{}"}}"#, sub));
    format!("{}.{}.fakesig", header, payload)
}

#[tokio::test]
async fn test_import_service_loads_encrypted_credentials_from_sqlite_accounts() {
    let _guard = ENV_LOCK.lock().unwrap();

    // Ensure environment variables are NOT set so we prove it loads strictly from DB
    std::env::remove_var("TIDAL_ACCESS_TOKEN");
    std::env::remove_var("TIDAL_USER_ID");
    std::env::remove_var("TIDAL_COUNTRY_CODE");

    let pool = setup_test_db().await;

    // Fetch tidal service ID
    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("tidal service must exist in migrated DB");

    let creds_json = serde_json::json!({
        "access_token": "tidal_dyn_token_abc123",
        "user_id": "real_tidal_user_789",
        "country_code": "DE"
    })
    .to_string();

    let encrypted = crypto::encrypt(&creds_json).expect("encryption must succeed");

    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, credentials_json, credentials_invalid, is_active) \
         VALUES (?, 'Tidal Pro User', ?, 0, 1)",
    )
    .bind(tidal_svc_id)
    .bind(encrypted)
    .execute(&pool)
    .await
    .expect("insert account must succeed");

    // Spawn mock server returning empty favorites
    let empty_favorites = serde_json::json!({
        "totalNumberOfItems": 0,
        "items": []
    })
    .to_string();
    let (mock_url, req_log) = spawn_mock_tidal(empty_favorites).await;
    std::env::set_var("TIDAL_API_BASE_URL", &mock_url);

    let app = create_test_app(pool);
    let state = app.state::<AppState>();

    // Call import_service
    let res = import_service("tidal".to_string(), state).await;

    // Clean up env
    std::env::remove_var("TIDAL_API_BASE_URL");

    assert!(res.is_ok(), "import_service should succeed: {:?}", res.err());
    let msg = res.unwrap();
    assert!(msg.contains("Tidal: 0 imported, 0 skipped"));

    // Verify mock server received request with user ID and token from DB
    let requests = req_log.lock().unwrap();
    assert!(!requests.is_empty(), "Mock server must receive request");
    let req = &requests[0];
    assert!(
        req.target.contains("/users/real_tidal_user_789/favorites/tracks"),
        "Target url must contain dynamic user_id: {}",
        req.target
    );
    assert!(
        req.target.contains("countryCode=DE"),
        "Target url must contain countryCode=DE: {}",
        req.target
    );
    assert!(
        req.auth_header
            .as_deref()
            .unwrap_or("")
            .contains("Bearer tidal_dyn_token_abc123"),
        "Authorization header must contain dynamic token: {:?}",
        req.auth_header
    );
}

#[tokio::test]
async fn test_import_service_plaintext_credentials_fallback() {
    let _guard = ENV_LOCK.lock().unwrap();

    std::env::remove_var("TIDAL_ACCESS_TOKEN");
    std::env::remove_var("TIDAL_USER_ID");
    std::env::remove_var("TIDAL_COUNTRY_CODE");

    let pool = setup_test_db().await;

    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("tidal service must exist");

    // Store plaintext JSON directly
    let creds_json = serde_json::json!({
        "access_token": "plaintext_tok_555",
        "user_id": "plain_user_666",
        "country_code": "CA"
    })
    .to_string();

    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, credentials_json, credentials_invalid, is_active) \
         VALUES (?, 'Plaintext User', ?, 0, 1)",
    )
    .bind(tidal_svc_id)
    .bind(creds_json)
    .execute(&pool)
    .await
    .expect("insert account must succeed");

    let empty_favorites = serde_json::json!({
        "totalNumberOfItems": 0,
        "items": []
    })
    .to_string();
    let (mock_url, req_log) = spawn_mock_tidal(empty_favorites).await;
    std::env::set_var("TIDAL_API_BASE_URL", &mock_url);

    let app = create_test_app(pool);
    let state = app.state::<AppState>();

    let res = import_service("tidal".to_string(), state).await;
    std::env::remove_var("TIDAL_API_BASE_URL");

    assert!(res.is_ok(), "import_service should succeed: {:?}", res.err());

    let requests = req_log.lock().unwrap();
    assert!(!requests.is_empty());
    assert!(requests[0].target.contains("/users/plain_user_666/favorites/tracks"));
    assert!(requests[0].target.contains("countryCode=CA"));
    assert!(requests[0]
        .auth_header
        .as_deref()
        .unwrap_or("")
        .contains("Bearer plaintext_tok_555"));
}

#[tokio::test]
async fn test_import_service_user_id_as_number() {
    let _guard = ENV_LOCK.lock().unwrap();

    let pool = setup_test_db().await;
    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("tidal service");

    // user_id as numeric integer in JSON
    let creds_json = serde_json::json!({
        "access_token": "num_tok_111",
        "user_id": 987654321,
        "country_code": "JP"
    })
    .to_string();

    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, credentials_json, is_active) VALUES (?, 'Num User', ?, 1)"
    )
    .bind(tidal_svc_id)
    .bind(creds_json)
    .execute(&pool)
    .await
    .expect("insert");

    let (id, token, uid, country) = resolve_tidal_import_credentials(&pool)
        .await
        .expect("must resolve numeric user_id");

    assert!(id > 0);
    assert_eq!(token, "num_tok_111");
    assert_eq!(uid, "987654321");
    assert_eq!(country, "JP");
}

#[tokio::test]
async fn test_import_service_user_id_extracted_from_jwt() {
    let _guard = ENV_LOCK.lock().unwrap();

    let pool = setup_test_db().await;
    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("tidal service");

    let jwt = make_test_jwt("jwt_extracted_user_888");

    // No explicit user_id in JSON payload — only JWT access_token
    let creds_json = serde_json::json!({
        "access_token": jwt
    })
    .to_string();

    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, credentials_json, is_active) VALUES (?, 'JWT User', ?, 1)"
    )
    .bind(tidal_svc_id)
    .bind(creds_json)
    .execute(&pool)
    .await
    .expect("insert");

    let (_, _, uid, _) = resolve_tidal_import_credentials(&pool)
        .await
        .expect("must extract user_id from JWT");

    assert_eq!(uid, "jwt_extracted_user_888");
}

#[tokio::test]
async fn test_extract_user_id_from_jwt_helper() {
    let jwt = make_test_jwt("sub_user_42");
    assert_eq!(extract_user_id_from_jwt(&jwt), Some("sub_user_42".to_string()));

    assert_eq!(extract_user_id_from_jwt("not.a.valid.jwt"), None);
    assert_eq!(extract_user_id_from_jwt("invalid"), None);
}

#[tokio::test]
async fn test_import_service_no_account_returns_friendly_error() {
    let _guard = ENV_LOCK.lock().unwrap();

    std::env::remove_var("TIDAL_ACCESS_TOKEN");
    std::env::remove_var("TIDAL_USER_ID");

    let pool = setup_test_db().await;
    let app = create_test_app(pool);
    let state = app.state::<AppState>();

    let res = import_service("tidal".to_string(), state).await;
    assert!(res.is_err(), "Should fail when no account exists");
    let err = res.unwrap_err();
    assert!(
        err.contains("No active account found for service tidal"),
        "Error message must be friendly: {}",
        err
    );
}

#[tokio::test]
async fn test_import_service_invalid_credentials_returns_requires_auth() {
    let _guard = ENV_LOCK.lock().unwrap();

    let pool = setup_test_db().await;
    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("tidal service");

    let creds_json = serde_json::json!({
        "access_token": "expired_tok",
        "user_id": "expired_usr"
    })
    .to_string();

    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, credentials_json, credentials_invalid, is_active) \
         VALUES (?, 'Invalid User', ?, 1, 1)"
    )
    .bind(tidal_svc_id)
    .bind(creds_json)
    .execute(&pool)
    .await
    .expect("insert");

    let app = create_test_app(pool);
    let state = app.state::<AppState>();

    let res = import_service("tidal".to_string(), state).await;
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("RequiresAuth"), "Error must require auth: {}", err);
}

#[tokio::test]
async fn test_dev_fallback_without_user_id_rejected() {
    let _guard = ENV_LOCK.lock().unwrap();

    let pool = setup_test_db().await;

    // No account in DB, but TIDAL_ACCESS_TOKEN is set as opaque non-JWT string and NO TIDAL_USER_ID
    std::env::set_var("TIDAL_ACCESS_TOKEN", "opaque_test_token");
    std::env::remove_var("TIDAL_USER_ID");

    let res = resolve_tidal_import_credentials(&pool).await;

    std::env::remove_var("TIDAL_ACCESS_TOKEN");

    assert!(res.is_err(), "Should reject fallback without configured user ID");
    let err = res.unwrap_err();
    assert!(
        err.contains("TIDAL_USER_ID not configured"),
        "Must require user ID and not use hardcoded mock: {}",
        err
    );
}

#[tokio::test]
async fn test_dev_fallback_with_env_vars_succeeds() {
    let _guard = ENV_LOCK.lock().unwrap();

    let pool = setup_test_db().await;

    std::env::set_var("TIDAL_ACCESS_TOKEN", "dev_token_123");
    std::env::set_var("TIDAL_USER_ID", "custom_dev_user_999");
    std::env::set_var("TIDAL_COUNTRY_CODE", "FR");

    let res = resolve_tidal_import_credentials(&pool).await;

    std::env::remove_var("TIDAL_ACCESS_TOKEN");
    std::env::remove_var("TIDAL_USER_ID");
    std::env::remove_var("TIDAL_COUNTRY_CODE");

    assert!(res.is_ok(), "Fallback with explicit user ID should succeed");
    let (id, token, uid, country) = res.unwrap();
    assert!(id > 0);
    assert_eq!(token, "dev_token_123");
    assert_eq!(uid, "custom_dev_user_999");
    assert_eq!(country, "FR");
}
