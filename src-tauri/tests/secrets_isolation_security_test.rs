//! Secrets Isolation and Credentials Security Test Suite (TASK-94 / SEC-010)
//!
//! Validates:
//! 1. Elimination of hardcoded official static secrets across source code:
//!    - Spotify developer credentials purged from `.env` and `.env.example`.
//!    - Official Qobuz credentials removed from `src-tauri/src/services/qobuz.rs` and `src-tauri/src/commands/migration.rs`.
//!    - Official Tidal credentials and base64 obfuscations removed from `crates/syncify-tidal-downloader/src/lib.rs`.
//!    - Hardcoded Blowfish decryption key removed from `scripts/services/deezer_service.py`.
//! 2. Safe resolution of credentials from environment variables with fallback dev placeholders.
//! 3. Verify `.env` is gitignored.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use syncify_tauri_lib::services::qobuz::{
    get_qobuz_app_id, get_qobuz_app_secret, QOBUZ_APP_ID, QOBUZ_APP_ID_FALLBACK,
    QOBUZ_APP_SECRET, QOBUZ_APP_SECRET_FALLBACK,
};
use syncify_tidal_downloader::{
    TidalDownloader, TidalGuiCredentials, DEFAULT_TIDAL_CLIENT_ID_FALLBACK,
    DEFAULT_TIDAL_CLIENT_SECRET_FALLBACK,
};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn get_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Repo root must be parent of src-tauri")
        .to_path_buf()
}

// Known sensitive strings that must NEVER appear hardcoded in source files
const FORBIDDEN_SPOTIFY_SECRET: &str = "ef45c66f47c94e829bcb85c3ce77da87";
const FORBIDDEN_SPOTIFY_CLIENT_ID: &str = "2e875ba784df4889806776da4a6f5bfd";
const FORBIDDEN_QOBUZ_APP_ID: &str = "798273057";
const FORBIDDEN_QOBUZ_SECRET: &str = "abb21364945c0583309667d13ca3d93a";
const FORBIDDEN_TIDAL_CLIENT_ID: &str = "fX2JxdmntZWK0ixT";
const FORBIDDEN_TIDAL_SECRET: &str = "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8=";
const FORBIDDEN_TIDAL_B64_CLIENT_ID: &str = "NkJEU1JkcEs5aHFFQlRnVQ==";
const FORBIDDEN_TIDAL_B64_SECRET: &str = "eGV1UG1ZN25icFo5SUliTEFjUTkzc2hrYTFWTmhlVUFxTjZJY3N6alRHOD0=";
const FORBIDDEN_DEEZER_BLOWFISH_KEY: &str = "g4el58wc0zvf9na1";

#[test]
fn test_env_files_and_gitignore_hygiene() {
    let repo_root = get_repo_root();

    // Check .gitignore
    let gitignore_path = repo_root.join(".gitignore");
    assert!(gitignore_path.exists(), ".gitignore must exist");
    let gitignore_content = fs::read_to_string(&gitignore_path).expect("Read .gitignore");
    assert!(
        gitignore_content.lines().any(|line| line.trim() == ".env"),
        ".gitignore must contain an exact line for .env"
    );

    // Check .env
    let env_path = repo_root.join(".env");
    if env_path.exists() {
        let env_content = fs::read_to_string(&env_path).expect("Read .env");
        assert!(
            !env_content.contains(FORBIDDEN_SPOTIFY_SECRET),
            ".env must NOT contain the real Spotify client secret"
        );
        assert!(
            !env_content.contains(FORBIDDEN_SPOTIFY_CLIENT_ID),
            ".env must NOT contain the hardcoded Spotify client ID"
        );
        assert!(
            !env_content.contains(FORBIDDEN_QOBUZ_APP_ID),
            ".env must NOT contain the official Qobuz app ID"
        );
        assert!(
            !env_content.contains(FORBIDDEN_QOBUZ_SECRET),
            ".env must NOT contain the official Qobuz secret"
        );
        assert!(
            !env_content.contains(FORBIDDEN_TIDAL_CLIENT_ID),
            ".env must NOT contain the official Tidal client ID"
        );
        assert!(
            !env_content.contains(FORBIDDEN_TIDAL_SECRET),
            ".env must NOT contain the official Tidal secret"
        );
        assert!(
            !env_content.contains(FORBIDDEN_DEEZER_BLOWFISH_KEY),
            ".env must NOT contain the official Deezer blowfish key"
        );
        assert!(
            env_content.contains("your_spotify_client_secret_here"),
            ".env must contain instructional placeholder for SPOTIPY_CLIENT_SECRET"
        );
    }

    // Check .env.example
    let env_example_path = repo_root.join(".env.example");
    assert!(env_example_path.exists(), ".env.example must exist");
    let example_content = fs::read_to_string(&env_example_path).expect("Read .env.example");
    assert!(
        !example_content.contains(FORBIDDEN_SPOTIFY_SECRET),
        ".env.example must NOT contain real Spotify client secret"
    );
    assert!(
        !example_content.contains(FORBIDDEN_SPOTIFY_CLIENT_ID),
        ".env.example must NOT contain real Spotify client ID"
    );
    assert!(
        !example_content.contains(FORBIDDEN_QOBUZ_APP_ID),
        ".env.example must NOT contain official Qobuz app ID"
    );
    assert!(
        !example_content.contains(FORBIDDEN_QOBUZ_SECRET),
        ".env.example must NOT contain official Qobuz secret"
    );
    assert!(
        !example_content.contains(FORBIDDEN_TIDAL_CLIENT_ID),
        ".env.example must NOT contain official Tidal client ID"
    );
    assert!(
        !example_content.contains(FORBIDDEN_TIDAL_SECRET),
        ".env.example must NOT contain official Tidal secret"
    );
    assert!(
        !example_content.contains(FORBIDDEN_DEEZER_BLOWFISH_KEY),
        ".env.example must NOT contain official Deezer blowfish key"
    );
    assert!(
        example_content.contains("your_spotify_client_secret_here"),
        ".env.example must contain instructional placeholder"
    );
}

#[test]
fn test_qobuz_source_files_free_of_hardcoded_secrets() {
    let repo_root = get_repo_root();

    let qobuz_rs = repo_root.join("src-tauri").join("src").join("services").join("qobuz.rs");
    assert!(qobuz_rs.exists(), "src-tauri/src/services/qobuz.rs must exist");
    let content = fs::read_to_string(&qobuz_rs).expect("Read qobuz.rs");

    assert!(
        !content.contains(FORBIDDEN_QOBUZ_APP_ID),
        "qobuz.rs must NOT contain the official QOBUZ_APP_ID string"
    );
    assert!(
        !content.contains(FORBIDDEN_QOBUZ_SECRET),
        "qobuz.rs must NOT contain the official QOBUZ_APP_SECRET string"
    );
    assert!(
        content.contains("QOBUZ_APP_ID_FALLBACK"),
        "qobuz.rs must define QOBUZ_APP_ID_FALLBACK"
    );
    assert!(
        content.contains("get_qobuz_app_id"),
        "qobuz.rs must provide dynamic get_qobuz_app_id helper"
    );

    let migration_rs = repo_root.join("src-tauri").join("src").join("commands").join("migration.rs");
    if migration_rs.exists() {
        let migration_content = fs::read_to_string(&migration_rs).expect("Read migration.rs");
        assert!(
            !migration_content.contains(FORBIDDEN_QOBUZ_APP_ID),
            "migration.rs must NOT hardcode the official QOBUZ_APP_ID"
        );
    }
}

#[test]
fn test_qobuz_dynamic_credentials_resolution() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let prev_id = std::env::var("QOBUZ_APP_ID").ok();
    let prev_secret = std::env::var("QOBUZ_APP_SECRET").ok();

    // 1. When environment variables are unset, fallback to development placeholders
    std::env::remove_var("QOBUZ_APP_ID");
    std::env::remove_var("QOBUZ_APP_SECRET");

    assert_eq!(get_qobuz_app_id(), QOBUZ_APP_ID_FALLBACK);
    assert_eq!(get_qobuz_app_secret(), QOBUZ_APP_SECRET_FALLBACK);
    assert_eq!(QOBUZ_APP_ID, QOBUZ_APP_ID_FALLBACK);
    assert_eq!(QOBUZ_APP_SECRET, QOBUZ_APP_SECRET_FALLBACK);

    // 2. When environment variables are set, dynamic getters resolve them
    std::env::set_var("QOBUZ_APP_ID", "custom_test_qobuz_id_999");
    std::env::set_var("QOBUZ_APP_SECRET", "custom_test_qobuz_secret_xyz");

    assert_eq!(get_qobuz_app_id(), "custom_test_qobuz_id_999");
    assert_eq!(get_qobuz_app_secret(), "custom_test_qobuz_secret_xyz");

    // Restore environment
    match prev_id {
        Some(v) => std::env::set_var("QOBUZ_APP_ID", v),
        None => std::env::remove_var("QOBUZ_APP_ID"),
    }
    match prev_secret {
        Some(v) => std::env::set_var("QOBUZ_APP_SECRET", v),
        None => std::env::remove_var("QOBUZ_APP_SECRET"),
    }
}

#[test]
fn test_tidal_source_files_free_of_hardcoded_secrets() {
    let repo_root = get_repo_root();
    let tidal_lib = repo_root
        .join("crates")
        .join("syncify-tidal-downloader")
        .join("src")
        .join("lib.rs");

    assert!(tidal_lib.exists(), "syncify-tidal-downloader/src/lib.rs must exist");
    let content = fs::read_to_string(&tidal_lib).expect("Read tidal lib.rs");

    assert!(
        !content.contains(FORBIDDEN_TIDAL_CLIENT_ID),
        "syncify-tidal-downloader/src/lib.rs must NOT contain official Tidal CLIENT_ID"
    );
    assert!(
        !content.contains(FORBIDDEN_TIDAL_SECRET),
        "syncify-tidal-downloader/src/lib.rs must NOT contain official Tidal CLIENT_SECRET"
    );
    assert!(
        !content.contains(FORBIDDEN_TIDAL_B64_CLIENT_ID),
        "syncify-tidal-downloader/src/lib.rs must NOT contain base64-encoded Tidal client ID"
    );
    assert!(
        !content.contains(FORBIDDEN_TIDAL_B64_SECRET),
        "syncify-tidal-downloader/src/lib.rs must NOT contain base64-encoded Tidal secret"
    );
    assert!(
        content.contains("TIDAL_CLIENT_ID"),
        "syncify-tidal-downloader/src/lib.rs must resolve TIDAL_CLIENT_ID from environment"
    );
    assert!(
        content.contains("TIDAL_CLIENT_SECRET"),
        "syncify-tidal-downloader/src/lib.rs must resolve TIDAL_CLIENT_SECRET from environment"
    );
}

#[test]
fn test_tidal_credentials_resolution() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let prev_id = std::env::var("TIDAL_CLIENT_ID").ok();
    let prev_secret = std::env::var("TIDAL_CLIENT_SECRET").ok();

    // 1. Without env vars and with None in struct: fallback to placeholders
    std::env::remove_var("TIDAL_CLIENT_ID");
    std::env::remove_var("TIDAL_CLIENT_SECRET");

    let default_creds = TidalGuiCredentials {
        access_token: "tok".to_string(),
        refresh_token: None,
        token_expiry: None,
        expires_at: None,
        expires_in: None,
        user_id: None,
        country_code: None,
        client_id: None,
        client_secret: None,
    };

    assert_eq!(default_creds.get_client_id().as_ref(), DEFAULT_TIDAL_CLIENT_ID_FALLBACK);
    assert_eq!(default_creds.get_client_secret().as_ref(), DEFAULT_TIDAL_CLIENT_SECRET_FALLBACK);

    // 2. With environment variables set:
    std::env::set_var("TIDAL_CLIENT_ID", "env_tidal_id_123");
    std::env::set_var("TIDAL_CLIENT_SECRET", "env_tidal_secret_456");

    assert_eq!(default_creds.get_client_id().as_ref(), "env_tidal_id_123");
    assert_eq!(default_creds.get_client_secret().as_ref(), "env_tidal_secret_456");

    // 3. With explicit credentials in struct: takes precedence over env vars
    let custom_creds = TidalGuiCredentials {
        access_token: "tok".to_string(),
        refresh_token: None,
        token_expiry: None,
        expires_at: None,
        expires_in: None,
        user_id: None,
        country_code: None,
        client_id: Some("explicit_override_id".to_string()),
        client_secret: Some("explicit_override_secret".to_string()),
    };

    assert_eq!(custom_creds.get_client_id().as_ref(), "explicit_override_id");
    assert_eq!(custom_creds.get_client_secret().as_ref(), "explicit_override_secret");

    // 4. Test TidalDownloader constructor
    let downloader = TidalDownloader::new();
    // Default downloader picks up env vars when available
    drop(downloader);

    let explicit_downloader = TidalDownloader::with_credentials(
        "inj_id".to_string(),
        "inj_secret".to_string(),
    );
    drop(explicit_downloader);

    // Restore environment
    match prev_id {
        Some(v) => std::env::set_var("TIDAL_CLIENT_ID", v),
        None => std::env::remove_var("TIDAL_CLIENT_ID"),
    }
    match prev_secret {
        Some(v) => std::env::set_var("TIDAL_CLIENT_SECRET", v),
        None => std::env::remove_var("TIDAL_CLIENT_SECRET"),
    }
}

#[test]
fn test_deezer_service_source_free_of_hardcoded_blowfish_secret() {
    let repo_root = get_repo_root();
    let deezer_py = repo_root
        .join("scripts")
        .join("services")
        .join("deezer_service.py");

    assert!(deezer_py.exists(), "scripts/services/deezer_service.py must exist");
    let content = fs::read_to_string(&deezer_py).expect("Read deezer_service.py");

    assert!(
        !content.contains(FORBIDDEN_DEEZER_BLOWFISH_KEY),
        "deezer_service.py must NOT contain the hardcoded Blowfish key string"
    );
    assert!(
        content.contains("DEEZER_BLOWFISH_KEY"),
        "deezer_service.py must support DEEZER_BLOWFISH_KEY environment variable"
    );
    assert!(
        content.contains("resolve_blowfish_key"),
        "deezer_service.py must implement dynamic resolve_blowfish_key method"
    );
}
