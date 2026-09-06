/// SEC-001 / TASK-85: Elimination of Plaintext Credentials Persistence Security Tests
///
/// Validates:
/// 1. `auth.rs` does not contain insecure plaintext dotfile reads (.gui_credentials_cache.json, .gui_settings.json)
///    and does not expose or call `load_qobuz_cache_fallback_auth`.
/// 2. `qobuz_auth.py` does not serialize or persist passwords or session tokens to plaintext disk files.
/// 3. `deezer_auth.py` does not dump ARL tokens or credentials to plaintext disk files.
/// 4. Qobuz and Tidal credential fallback uses the canonical AES-256-GCM encrypted SQLite database.
/// 5. Account credentials stored in SQLite are encrypted and never stored in plaintext JSON.

use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use syncify_tauri_lib::commands::{load_qobuz_db_fallback_auth, load_tidal_db_cached_token_expiry};
use syncify_tauri_lib::crypto;

fn get_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest_dir)
}

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([0x5A; 32]);

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

// ── Test 1: Source code hygiene in auth.rs ───────────────────────────────────

#[test]
fn test_auth_rs_source_hygiene_no_plaintext_dotfile_reads() {
    let workspace_root = get_workspace_root();
    let auth_rs_path = workspace_root.join("src-tauri/src/commands/auth.rs");
    assert!(
        auth_rs_path.exists(),
        "auth.rs must exist at {:?}",
        auth_rs_path
    );

    let content = fs::read_to_string(&auth_rs_path).expect("read auth.rs");

    // Insecure cache loader must be completely removed
    assert!(
        !content.contains("load_qobuz_cache_fallback_auth"),
        "auth.rs must NOT contain load_qobuz_cache_fallback_auth"
    );

    // Insecure dotfile references must not exist in auth.rs
    assert!(
        !content.contains(".gui_credentials_cache.json"),
        "auth.rs must NOT reference .gui_credentials_cache.json"
    );
    assert!(
        !content.contains(".gui_settings.json"),
        "auth.rs must NOT reference .gui_settings.json"
    );
    assert!(
        !content.contains("load_tidal_cached_token_expiry"),
        "auth.rs must NOT contain load_tidal_cached_token_expiry"
    );

    // Canonical database fallbacks must be present
    assert!(
        content.contains("load_qobuz_db_fallback_auth"),
        "auth.rs must use load_qobuz_db_fallback_auth"
    );
    assert!(
        content.contains("load_tidal_db_cached_token_expiry"),
        "auth.rs must use load_tidal_db_cached_token_expiry"
    );
}

// ── Test 2: Qobuz Python script does not persist passwords or credentials to disk ─

#[test]
fn test_python_qobuz_auth_does_not_persist_passwords_or_credentials_to_disk() {
    let workspace_root = get_workspace_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let fake_cache_file = temp_dir.path().join(".gui_credentials_cache.json");

    let python_script = format!(
        r#"
import sys
from pathlib import Path

repo_root = Path(r"{}")
sys.path.insert(0, str(repo_root / "scripts"))

from services.qobuz_auth import QobuzAuth

fake_cache = Path(r"{}")
auth = QobuzAuth(credentials_file=fake_cache, verbose=False)

# 1. Test save_session does not write password or session to disk
secret_password = "super_secret_qobuz_password_12345"
session_payload = {{
    "user_id": "test_user_42",
    "auth_token": "valid_token_hex_1234567890abcdef",
    "username": "tester@syncify.io",
    "password": secret_password,
}}

auth.save_session(session_payload)

if fake_cache.exists():
    content = fake_cache.read_text()
    if secret_password in content:
        print("ERROR: Password leaked to disk file!", file=sys.stderr)
        sys.exit(1)
    if "valid_token_hex_1234567890abcdef" in content:
        print("ERROR: Auth token leaked to disk file!", file=sys.stderr)
        sys.exit(2)

# Verify in-memory session was retained for ephemeral stdout transport
stored = auth.get_stored_session()
assert stored is not None, "In-memory session should be available"
assert stored.get("password") == secret_password, "In-memory session password should match"

print("SUCCESS")
"#,
        workspace_root.display(),
        fake_cache_file.display()
    );

    let output = Command::new("python3")
        .arg("-c")
        .arg(&python_script)
        .output()
        .expect("execute python script");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Python Qobuz auth check failed: stdout={}, stderr={}",
        stdout,
        stderr
    );
    assert!(stdout.contains("SUCCESS"));

    // Ensure the cache file does not exist or does not contain any plaintext password
    if fake_cache_file.exists() {
        let text = fs::read_to_string(&fake_cache_file).unwrap();
        assert!(!text.contains("super_secret_qobuz_password_12345"));
    }
}

// ── Test 3: Deezer Python script does not persist ARL or credentials to disk ──────

#[test]
fn test_python_deezer_auth_does_not_persist_arl_to_disk() {
    let workspace_root = get_workspace_root();
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let fake_cache_file = temp_dir.path().join(".gui_credentials_cache.json");

    let python_script = format!(
        r#"
import sys
from pathlib import Path

repo_root = Path(r"{}")
sys.path.insert(0, str(repo_root / "scripts"))

from services.deezer_auth import DeezerAuth

fake_cache = Path(r"{}")
auth = DeezerAuth(credentials_file=fake_cache, verbose=False)

secret_arl = "secret_arl_cookie_token_999888777666555444333222111"
auth.save_arl(secret_arl)

if fake_cache.exists():
    content = fake_cache.read_text()
    if secret_arl in content:
        print("ERROR: Deezer ARL leaked to disk file!", file=sys.stderr)
        sys.exit(1)

# Verify in-memory ARL was retained for ephemeral stdout transport
stored = auth.get_stored_arl()
assert stored == secret_arl, "In-memory ARL should be available"

print("SUCCESS")
"#,
        workspace_root.display(),
        fake_cache_file.display()
    );

    let output = Command::new("python3")
        .arg("-c")
        .arg(&python_script)
        .output()
        .expect("execute python script");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Python Deezer auth check failed: stdout={}, stderr={}",
        stdout,
        stderr
    );
    assert!(stdout.contains("SUCCESS"));

    if fake_cache_file.exists() {
        let text = fs::read_to_string(&fake_cache_file).unwrap();
        assert!(!text.contains("secret_arl_cookie_token_999888777666555444333222111"));
    }
}

// ── Test 4: SQLite credentials persistence is encrypted with AES-256-GCM ─────────

#[tokio::test]
async fn test_sqlite_accounts_persistence_is_encrypted_and_db_fallback_works() {
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("qobuz service row must exist");

    let raw_secret_password = "very_sensitive_plaintext_password_777!";
    let raw_auth_token = "valid_qobuz_token_abc123456789";

    let creds_json = serde_json::json!({
        "user_auth_token": raw_auth_token,
        "auth_token": raw_auth_token,
        "username": "qobuz_subscriber@example.com",
        "password": raw_secret_password,
        "user_id": "1234567"
    })
    .to_string();

    // Encrypt with canonical crypto module (AES-256-GCM)
    let encrypted_payload = crypto::encrypt(&creds_json).expect("encryption succeeds");

    // Insert account
    sqlx::query(
        r#"
        INSERT INTO accounts
            (service_id, display_name, email, credentials_json, credentials_invalid, is_active, created_at)
        VALUES (?, ?, ?, ?, 0, 1, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(qobuz_svc_id)
    .bind("Qobuz Test User")
    .bind("qobuz_subscriber@example.com")
    .bind(&encrypted_payload)
    .execute(&pool)
    .await
    .expect("insert account succeeds");

    // Verify raw database record:
    let stored_record: (String,) = sqlx::query_as("SELECT credentials_json FROM accounts WHERE service_id = ?")
        .bind(qobuz_svc_id)
        .fetch_one(&pool)
        .await
        .expect("fetch account");

    let raw_stored = &stored_record.0;

    // 1. Raw stored string must NOT contain plaintext password
    assert!(
        !raw_stored.contains(raw_secret_password),
        "SQLite credentials_json must NOT contain plaintext password"
    );
    // 2. Raw stored string must NOT be raw JSON
    assert!(
        !raw_stored.trim().starts_with('{'),
        "SQLite credentials_json must be encrypted ciphertext, not plain JSON"
    );

    // 3. Decrypting with crypto::decrypt successfully restores the credentials
    let decrypted = crypto::decrypt(raw_stored).expect("decryption succeeds");
    assert!(decrypted.contains(raw_secret_password));
    assert!(decrypted.contains(raw_auth_token));

    // 4. Test canonical load_qobuz_db_fallback_auth:
    let (fb_token, fb_username, fb_password) = load_qobuz_db_fallback_auth(&pool).await;
    assert_eq!(fb_token.as_deref(), Some(raw_auth_token));
    assert_eq!(fb_username.as_deref(), Some("qobuz_subscriber@example.com"));
    assert_eq!(fb_password.as_deref(), Some(raw_secret_password));
}

// ── Test 5: SQLite Tidal token expiry fallback works from encrypted DB ────────────

#[tokio::test]
async fn test_sqlite_tidal_token_expiry_db_fallback_works() {
    let pool = setup_test_db().await;

    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("tidal service row must exist");

    let expected_expiry = 1893456000.0; // Future timestamp
    let creds_json = serde_json::json!({
        "access_token": "tidal_access_token_12345",
        "refresh_token": "tidal_refresh_token_67890",
        "token_expiry": expected_expiry
    })
    .to_string();

    let encrypted_payload = crypto::encrypt(&creds_json).expect("encryption succeeds");

    sqlx::query(
        r#"
        INSERT INTO accounts
            (service_id, display_name, credentials_json, credentials_invalid, is_active, created_at)
        VALUES (?, ?, ?, 0, 1, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(tidal_svc_id)
    .bind("Tidal Test User")
    .bind(&encrypted_payload)
    .execute(&pool)
    .await
    .expect("insert account succeeds");

    let expiry = load_tidal_db_cached_token_expiry(&pool).await;
    assert_eq!(expiry, Some(expected_expiry));
}

// ── Test 6: Fallback returns None when no active account exists ───────────────────

#[tokio::test]
async fn test_db_fallback_returns_none_when_no_active_account() {
    let pool = setup_test_db().await;

    // Database is empty of accounts
    let (fb_token, fb_username, fb_password) = load_qobuz_db_fallback_auth(&pool).await;
    assert!(fb_token.is_none());
    assert!(fb_username.is_none());
    assert!(fb_password.is_none());

    let tidal_expiry = load_tidal_db_cached_token_expiry(&pool).await;
    assert!(tidal_expiry.is_none());
}
