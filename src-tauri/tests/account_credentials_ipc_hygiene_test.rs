//! account_credentials_ipc_hygiene_test.rs
//!
//! Regression test for [TASK-86] / [SEC-002]:
//! "Desregistro y Protección de Acceso Privado en Comando IPC get_account_credentials"
//!
//! Asserts that:
//! 1. `get_account_credentials` is completely removed from `main.rs` `generate_handler![]`.
//! 2. `get_internal_account_credentials` in `accounts.rs` does NOT carry the `#[tauri::command]` macro.
//! 3. No frontend file in `ui/` invokes `get_account_credentials`.
//! 4. The internal Rust backend function `get_internal_account_credentials` correctly:
//!    - Decrypts and returns legitimate account credentials.
//!    - Returns error for missing accounts.
//!    - Returns error for accounts with NULL credentials.
//!    - Handles corrupted credentials (AEAD errors) gracefully and auto-clears `credentials_json` to NULL.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::get_internal_account_credentials;
use syncify_tauri_lib::crypto;

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

#[test]
fn test_generate_handler_does_not_expose_get_account_credentials() {
    let main_rs = include_str!("../src/main.rs");

    let handler_start = main_rs
        .find("tauri::generate_handler![")
        .expect("tauri::generate_handler! must exist in main.rs");
    let handler_end = main_rs[handler_start..]
        .find("])")
        .expect("Closing delimiter for generate_handler! must exist");
    let handler_block = &main_rs[handler_start..handler_start + handler_end];

    assert!(
        !handler_block.contains("get_account_credentials"),
        "generate_handler! must NOT contain get_account_credentials (SEC-002)"
    );
    assert!(
        !handler_block.contains("get_internal_account_credentials"),
        "generate_handler! must NOT contain internal credentials function"
    );
}

#[test]
fn test_accounts_module_does_not_expose_credentials_as_tauri_command() {
    let accounts_rs = include_str!("../src/commands/accounts.rs");

    assert!(
        !accounts_rs.contains("#[tauri::command]\npub async fn get_account_credentials"),
        "get_account_credentials must not be a tauri::command"
    );
    assert!(
        !accounts_rs.contains("#[tauri::command]\r\npub async fn get_account_credentials"),
        "get_account_credentials must not be a tauri::command (crlf)"
    );

    // Verify get_internal_account_credentials does not have tauri::command attribute
    if let Some(idx) = accounts_rs.find("fn get_internal_account_credentials") {
        let preceding = &accounts_rs[..idx];
        let last_lines: Vec<&str> = preceding.lines().rev().take(5).collect();
        for line in last_lines {
            assert!(
                !line.contains("#[tauri::command]"),
                "get_internal_account_credentials must NOT have #[tauri::command]"
            );
        }
    } else {
        panic!("get_internal_account_credentials should exist in commands/accounts.rs");
    }
}

#[test]
fn test_ui_has_no_get_account_credentials_invocations() {
    let ui_src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Parent repo root")
        .join("ui")
        .join("src");

    if ui_src_dir.exists() {
        for entry in walkdir::WalkDir::new(&ui_src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().is_file()
                    && e.path()
                        .extension()
                        .map_or(false, |ext| ext == "ts" || ext == "tsx" || ext == "js" || ext == "jsx" || ext == "svelte" || ext == "vue")
            })
        {
            let content = std::fs::read_to_string(entry.path())
                .unwrap_or_default();
            assert!(
                !content.contains("get_account_credentials"),
                "Found get_account_credentials invocation in UI file: {:?}",
                entry.path()
            );
        }
    }
}

#[tokio::test]
async fn test_get_internal_account_credentials_success() {
    let pool = setup_test_db().await;

    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let dummy_plaintext = r#"{"user_auth_token":"mock_auth_token_value_xyz"}"#;
    let encrypted = crypto::encrypt(dummy_plaintext).expect("Encryption failed");

    let account_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active)
        VALUES (?, ?, 'Test User', 'user@example.com', 1)
        RETURNING id
        "#,
    )
    .bind(service_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .expect("Failed to insert account");

    let result = get_internal_account_credentials(&pool, account_id)
        .await
        .expect("Internal credentials retrieval should succeed");

    assert_eq!(result, dummy_plaintext);
}

#[tokio::test]
async fn test_get_internal_account_credentials_account_not_found() {
    let pool = setup_test_db().await;

    let result = get_internal_account_credentials(&pool, 99999).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Account not found");
}

#[tokio::test]
async fn test_get_internal_account_credentials_missing_creds() {
    let pool = setup_test_db().await;

    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active)
        VALUES (?, NULL, 'No Creds User', 'nocreds@example.com', 1)
        RETURNING id
        "#,
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .expect("Failed to insert account");

    let result = get_internal_account_credentials(&pool, account_id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Credentials missing"));
}

#[tokio::test]
async fn test_get_internal_account_credentials_corrupted_clears_db() {
    let pool = setup_test_db().await;

    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'spotify'")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Invalid base64/corrupted ciphertext that triggers error
    let corrupted_payload = "corrupted_non_aead_data_payload_string";

    let account_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active)
        VALUES (?, ?, 'Corrupt User', 'corrupt@example.com', 1)
        RETURNING id
        "#,
    )
    .bind(service_id)
    .bind(corrupted_payload)
    .fetch_one(&pool)
    .await
    .expect("Failed to insert account");

    let result = get_internal_account_credentials(&pool, account_id).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("expired") || err_msg.contains("reconnect"),
        "Error should guide user to reconnect account, got: {}",
        err_msg
    );

    // Verify DB cleared credentials_json to NULL
    let stored_creds: Option<String> = sqlx::query_scalar(
        "SELECT credentials_json FROM accounts WHERE id = ?"
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(stored_creds.is_none(), "Corrupted credentials should be cleared in DB");
}

#[tokio::test]
async fn test_get_internal_account_credentials_aead_tag_mismatch_clears_db() {
    let pool = setup_test_db().await;

    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Encrypt with an alternate key to simulate machine migration / keychain mismatch
    let alternate_key = [99u8; 32];
    let alternate_encrypted = crypto::encrypt_with_key(r#"{"auth_token":"stale"}"#, &alternate_key)
        .expect("Alternate key encryption");

    let account_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active)
        VALUES (?, ?, 'Key Mismatch User', 'keymismatch@example.com', 1)
        RETURNING id
        "#,
    )
    .bind(service_id)
    .bind(&alternate_encrypted)
    .fetch_one(&pool)
    .await
    .expect("Failed to insert account");

    let result = get_internal_account_credentials(&pool, account_id).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("expired") || err_msg.contains("reconnect"),
        "Error should guide user to reconnect account, got: {}",
        err_msg
    );

    // Verify DB cleared credentials_json to NULL
    let stored_creds: Option<String> = sqlx::query_scalar(
        "SELECT credentials_json FROM accounts WHERE id = ?"
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(stored_creds.is_none(), "Mismatched key credentials should be cleared in DB");
}
