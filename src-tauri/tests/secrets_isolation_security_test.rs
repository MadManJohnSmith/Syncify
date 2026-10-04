//! Política de credenciales (revisada 2026-10-04; sustituye a TASK-94 / SEC-010)
//!
//! Hay DOS clases de credenciales y este suite las distingue:
//!
//! 1. CREDENCIALES PÚBLICAS: identificadores de aplicación que el proveedor
//!    reconoce y que traen los clientes open-source. DEBEN estar en el código
//!    como valores por defecto, porque sin ellos el servicio se rompe (la API
//!    de Qobuz responde 400 "Invalid or missing app_id" con un placeholder).
//!    Quitarlos creyendo que eran secretos personales fue un error cometido
//!    dos veces; este suite ahora exige su presencia.
//!    - Qobuz: bundle público app_id/app_secret.
//!    - Tidal: client id/secret del cliente de escritorio + client id PKCE.
//!    - Deezer: clave Blowfish de descifrado de streams.
//! 2. CREDENCIALES PERSONALES: tokens, contraseñas, ARL y apps OAuth propias.
//!    ÚNICA EXCEPCIÓN DECLARADA POR EL PROYECTO: Spotify — sus políticas
//!    exigen una app registrada por el operador, así que sus credenciales son
//!    personales y NUNCA se incrustan en el repo (viven en el keychain, en la
//!    configuración de cuentas o en `.env` gitignored).
//!
//! En todos los casos, las credenciales guardadas y las variables de entorno
//! tienen prioridad sobre los valores públicos por defecto.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use syncify_tauri_lib::services::qobuz::{
    get_qobuz_app_id, get_qobuz_app_secret, QOBUZ_APP_ID, QOBUZ_APP_ID_FALLBACK, QOBUZ_APP_SECRET,
    QOBUZ_APP_SECRET_FALLBACK,
};
use syncify_tidal_downloader::{
    TidalDownloader, TidalGuiCredentials, DEFAULT_TIDAL_CLIENT_ID_FALLBACK,
    DEFAULT_TIDAL_CLIENT_ID_PKCE, DEFAULT_TIDAL_CLIENT_SECRET_FALLBACK,
};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn get_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Repo root must be parent of src-tauri")
        .to_path_buf()
}

// Credenciales personales que NUNCA deben aparecer en ficheros del repo.
const FORBIDDEN_SPOTIFY_SECRET: &str = "ef45c66f47c94e829bcb85c3ce77da87";
const FORBIDDEN_SPOTIFY_CLIENT_ID: &str = "2e875ba784df4889806776da4a6f5bfd";

// Credenciales PÚBLICAS que DEBEN existir en el código como valores por defecto.
const PUBLIC_QOBUZ_APP_ID: &str = "798273057";
const PUBLIC_QOBUZ_SECRET: &str = "abb21364945c0583309667d13ca3d93a";
const PUBLIC_TIDAL_CLIENT_ID: &str = "fX2JxdmntZWK0ixT";
const PUBLIC_TIDAL_SECRET: &str = "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8=";
const PUBLIC_TIDAL_CLIENT_ID_PKCE: &str = "6BDSRdpK9hqEBTgU";
const PUBLIC_DEEZER_BLOWFISH_KEY: &str = "g4el58wc0zvf9na1";

#[test]
fn test_env_files_and_gitignore_hygiene() {
    let repo_root = get_repo_root();

    let gitignore_path = repo_root.join(".gitignore");
    assert!(gitignore_path.exists(), ".gitignore must exist");
    let gitignore_content = fs::read_to_string(&gitignore_path).expect("Read .gitignore");
    assert!(
        gitignore_content.lines().any(|line| line.trim() == ".env"),
        ".gitignore must contain an exact line for .env"
    );

    // Solo credenciales PERSONALES prohibidas en ficheros de entorno; los
    // bundles públicos pueden aparecer (o no) porque no son secretos.
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
    }

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
    for variable in [
        "QOBUZ_APP_ID",
        "QOBUZ_APP_SECRET",
        "TIDAL_CLIENT_ID",
        "TIDAL_CLIENT_SECRET",
        "TIDAL_CLIENT_ID_PKCE",
        "DEEZER_BLOWFISH_KEY",
    ] {
        assert!(
            example_content.contains(variable),
            ".env.example must document {variable} (override del bundle público)"
        );
    }
}

#[test]
fn test_public_credentials_present_in_source() {
    let repo_root = get_repo_root();

    let qobuz_rs = fs::read_to_string(
        repo_root
            .join("src-tauri")
            .join("src")
            .join("services")
            .join("qobuz.rs"),
    )
    .expect("Read qobuz.rs");
    assert!(
        qobuz_rs.contains(PUBLIC_QOBUZ_APP_ID) && qobuz_rs.contains(PUBLIC_QOBUZ_SECRET),
        "qobuz.rs must embed the PUBLIC Qobuz app_id/secret as defaults; placeholdering them breaks the API with 400 (happened twice)"
    );
    assert!(
        qobuz_rs.contains("get_qobuz_app_id") && qobuz_rs.contains("QOBUZ_APP_ID_FALLBACK"),
        "qobuz.rs must keep the env-override resolution helpers"
    );

    let qobuz_py = fs::read_to_string(
        repo_root
            .join("scripts")
            .join("services")
            .join("qobuz_service.py"),
    )
    .expect("Read qobuz_service.py");
    assert!(
        qobuz_py.contains(PUBLIC_QOBUZ_APP_ID) && qobuz_py.contains(PUBLIC_QOBUZ_SECRET),
        "qobuz_service.py must embed the PUBLIC Qobuz bundle (the Python bridge already carried the public app_id)"
    );

    let tidal_lib = fs::read_to_string(
        repo_root
            .join("crates")
            .join("syncify-tidal-downloader")
            .join("src")
            .join("lib.rs"),
    )
    .expect("Read tidal lib.rs");
    assert!(
        tidal_lib.contains(PUBLIC_TIDAL_CLIENT_ID) && tidal_lib.contains(PUBLIC_TIDAL_SECRET),
        "syncify-tidal-downloader must embed the PUBLIC desktop client credentials"
    );
    assert!(
        tidal_lib.contains(PUBLIC_TIDAL_CLIENT_ID_PKCE),
        "syncify-tidal-downloader must embed the PUBLIC PKCE client id"
    );

    let tidal_auth_py = fs::read_to_string(
        repo_root
            .join("scripts")
            .join("services")
            .join("tidal_auth.py"),
    )
    .expect("Read tidal_auth.py");
    assert!(
        tidal_auth_py.contains(PUBLIC_TIDAL_CLIENT_ID)
            && tidal_auth_py.contains(PUBLIC_TIDAL_SECRET),
        "tidal_auth.py must embed the PUBLIC desktop client credentials"
    );
    assert!(
        tidal_auth_py.contains(PUBLIC_TIDAL_CLIENT_ID_PKCE),
        "tidal_auth.py must embed the PUBLIC PKCE client id"
    );
    for variable in [
        "TIDAL_CLIENT_ID",
        "TIDAL_CLIENT_SECRET",
        "TIDAL_CLIENT_ID_PKCE",
    ] {
        assert!(
            tidal_auth_py.contains(variable),
            "tidal_auth.py must resolve {variable} from the environment before the public default"
        );
    }

    let deezer_py = fs::read_to_string(
        repo_root
            .join("scripts")
            .join("services")
            .join("deezer_service.py"),
    )
    .expect("Read deezer_service.py");
    assert!(
        deezer_py.contains(PUBLIC_DEEZER_BLOWFISH_KEY),
        "deezer_service.py must embed the PUBLIC Blowfish key as the default"
    );
    assert!(
        deezer_py.contains("resolve_blowfish_key") && deezer_py.contains("DEEZER_BLOWFISH_KEY"),
        "deezer_service.py must keep the env/credentials override path"
    );

    let migration_rs = fs::read_to_string(
        repo_root
            .join("src-tauri")
            .join("src")
            .join("commands")
            .join("migration.rs"),
    )
    .expect("Read migration.rs");
    assert!(
        !migration_rs.contains(PUBLIC_QOBUZ_SECRET),
        "migration.rs must consume the constants from qobuz.rs, not duplicate the bundle"
    );
}

#[test]
fn test_personal_spotify_secrets_absent_from_source() {
    // Spotify es la ÚNICA excepción de credenciales personales (política
    // declarada del proyecto): sus credenciales viven en keychain/.env/cuentas.
    let repo_root = get_repo_root();
    let candidates = [
        repo_root.join("src-tauri").join("src"),
        repo_root.join("scripts").join("services"),
        repo_root.join("crates"),
    ];

    let mut scanned_files = 0;
    let mut violations = Vec::new();
    for base in &candidates {
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !matches!(ext, "rs" | "py" | "toml") {
                    continue;
                }
                if let Ok(content) = fs::read_to_string(&path) {
                    scanned_files += 1;
                    for forbidden in [FORBIDDEN_SPOTIFY_SECRET, FORBIDDEN_SPOTIFY_CLIENT_ID] {
                        if content.contains(forbidden) {
                            violations.push(format!(
                                "{}: contains a PERSONAL Spotify credential",
                                path.display()
                            ));
                        }
                    }
                }
            }
        }
    }

    assert!(
        scanned_files > 0,
        "Expected to scan source trees under src-tauri/src, scripts/services and crates"
    );
    assert!(
        violations.is_empty(),
        "Personal Spotify credentials leaked into source (scanned {scanned_files} files):\n{}",
        violations.join("\n")
    );
}

#[test]
fn test_qobuz_dynamic_credentials_resolution() {
    let _guard = ENV_MUTEX.lock().unwrap();

    let prev_id = std::env::var("QOBUZ_APP_ID").ok();
    let prev_secret = std::env::var("QOBUZ_APP_SECRET").ok();

    // 1. Sin variables de entorno: el bundle PÚBLICO es el valor por defecto.
    std::env::remove_var("QOBUZ_APP_ID");
    std::env::remove_var("QOBUZ_APP_SECRET");

    assert_eq!(get_qobuz_app_id(), PUBLIC_QOBUZ_APP_ID);
    assert_eq!(get_qobuz_app_secret(), PUBLIC_QOBUZ_SECRET);
    assert_eq!(QOBUZ_APP_ID_FALLBACK, PUBLIC_QOBUZ_APP_ID);
    assert_eq!(QOBUZ_APP_SECRET_FALLBACK, PUBLIC_QOBUZ_SECRET);
    assert_eq!(QOBUZ_APP_ID, QOBUZ_APP_ID_FALLBACK);
    assert_eq!(QOBUZ_APP_SECRET, QOBUZ_APP_SECRET_FALLBACK);

    // 2. Con variables de entorno: el override gana.
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
fn test_tidal_credentials_resolution() {
    let _guard = ENV_MUTEX.lock().unwrap();

    // Congela los valores públicos: restaurarlos es la política, perderlos el error.
    assert_eq!(DEFAULT_TIDAL_CLIENT_ID_FALLBACK, PUBLIC_TIDAL_CLIENT_ID);
    assert_eq!(DEFAULT_TIDAL_CLIENT_SECRET_FALLBACK, PUBLIC_TIDAL_SECRET);
    assert_eq!(DEFAULT_TIDAL_CLIENT_ID_PKCE, PUBLIC_TIDAL_CLIENT_ID_PKCE);

    let prev_id = std::env::var("TIDAL_CLIENT_ID").ok();
    let prev_secret = std::env::var("TIDAL_CLIENT_SECRET").ok();

    // 1. Sin env ni credenciales: el bundle público del cliente de escritorio.
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

    assert_eq!(
        default_creds.get_client_id().as_ref(),
        DEFAULT_TIDAL_CLIENT_ID_FALLBACK
    );
    assert_eq!(
        default_creds.get_client_secret().as_ref(),
        DEFAULT_TIDAL_CLIENT_SECRET_FALLBACK
    );

    // 2. Con variables de entorno: el override gana.
    std::env::set_var("TIDAL_CLIENT_ID", "env_tidal_id_123");
    std::env::set_var("TIDAL_CLIENT_SECRET", "env_tidal_secret_456");

    assert_eq!(default_creds.get_client_id().as_ref(), "env_tidal_id_123");
    assert_eq!(
        default_creds.get_client_secret().as_ref(),
        "env_tidal_secret_456"
    );

    // 3. Con credenciales explícitas en la estructura: prioridad máxima.
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

    assert_eq!(
        custom_creds.get_client_id().as_ref(),
        "explicit_override_id"
    );
    assert_eq!(
        custom_creds.get_client_secret().as_ref(),
        "explicit_override_secret"
    );

    // 4. El constructor por defecto no debe entrar en pánico.
    let downloader = TidalDownloader::new();
    drop(downloader);

    match prev_id {
        Some(v) => std::env::set_var("TIDAL_CLIENT_ID", v),
        None => std::env::remove_var("TIDAL_CLIENT_ID"),
    }
    match prev_secret {
        Some(v) => std::env::set_var("TIDAL_CLIENT_SECRET", v),
        None => std::env::remove_var("TIDAL_CLIENT_SECRET"),
    }
}
