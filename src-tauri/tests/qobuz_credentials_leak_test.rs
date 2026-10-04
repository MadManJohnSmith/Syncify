//! Contrato del bundle público de Qobuz (revisado 2026-10-04; sustituye a TASK-152 / SEC-025)
//!
//! El par app_id/app_secret de Qobuz es PÚBLICO: es el bundle que la propia
//! API exige y que traen los clientes open-source. Sin él la API responde 400
//! "Invalid or missing app_id". Sustituirlo por placeholders creyendo que era
//! un secreto personal rompió el servicio dos veces; este suite ahora:
//! 1. EXIGE el bundle en los ficheros canónicos donde vive.
//! 2. Detecta duplicaciones en cualquier otro fichero de producción (ahí sí
//!    sería una fuga innecesaria).
//! 3. Mantiene el contrato de override: los puentes Python leen
//!    QOBUZ_APP_ID / QOBUZ_APP_SECRET del entorno, que tienen prioridad.
//!
//! Las credenciales PERSONALES (token, login del usuario) siguen fuera del
//! árbol: keychain, cuentas o `.env` gitignored.

use std::fs;
use std::path::{Path, PathBuf};

fn get_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Repo root must be parent of src-tauri")
        .to_path_buf()
}

const PUBLIC_APP_ID: &str = "798273057";
const PUBLIC_SECRET: &str = "abb21364945c0583309667d13ca3d93a";
const PUBLIC_SECRET_PREFIX: &str = "abb21364";

/// Ficheros canónicos donde el bundle público debe estar incrustado.
const BUNDLE_CANONICAL_FILES: [&str; 3] = [
    "src-tauri/src/services/qobuz.rs",
    "scripts/services/qobuz_service.py",
    "scripts/services/qobuz_auth.py",
];

fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if !dir.exists() {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(&path, files);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
}

fn scan_tree(
    base: &Path,
    predicate: &dyn Fn(&str, &str) -> Option<String>,
) -> (usize, Vec<String>) {
    let mut files = Vec::new();
    collect_files_recursive(base, &mut files);
    let mut scanned = 0;
    let mut violations = Vec::new();
    for file in files {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "rs" | "toml" | "json" | "py" | "sh" | "md") {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&file) {
            scanned += 1;
            let relative = file
                .strip_prefix(get_repo_root())
                .unwrap_or(&file)
                .to_string_lossy()
                .to_string();
            if let Some(violation) = predicate(&relative, &content) {
                violations.push(violation);
            }
        }
    }
    (scanned, violations)
}

#[test]
fn test_public_bundle_present_in_canonical_files() {
    let repo_root = get_repo_root();
    for relative in BUNDLE_CANONICAL_FILES {
        let path = repo_root.join(relative);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Fichero canónico {relative} debe existir: {e}"));
        assert!(
            content.contains(PUBLIC_APP_ID),
            "{relative} debe incrustar el app_id público; un placeholder rompe la API con 400"
        );
    }
    // El secret solo se exige donde se firman peticiones (el app_id viaja en
    // URLs de consulta y no necesita firma).
    for relative in [
        "src-tauri/src/services/qobuz.rs",
        "scripts/services/qobuz_service.py",
    ] {
        let content = fs::read_to_string(repo_root.join(relative)).expect("Fichero canónico");
        assert!(
            content.contains(PUBLIC_SECRET),
            "{relative} debe incrustar el secret público del bundle para firmar"
        );
    }
}

#[test]
fn test_public_bundle_not_duplicated_outside_canonical_files() {
    let repo_root = get_repo_root();
    let candidates = [
        repo_root.join("scripts"),
        repo_root.join("src-tauri").join("src"),
    ];

    let mut scanned_files = 0;
    let mut violations = Vec::new();
    for base_dir in &candidates {
        let (scanned, mut tree_violations) = scan_tree(base_dir, &|relative, content| {
            if BUNDLE_CANONICAL_FILES.contains(&relative) {
                return None;
            }
            if content.contains(PUBLIC_APP_ID) {
                return Some(format!(
                    "{relative}: duplica el app_id público fuera de los ficheros canónicos"
                ));
            }
            if content.contains(PUBLIC_SECRET_PREFIX) {
                return Some(format!(
                    "{relative}: duplica el secret público fuera de los ficheros canónicos"
                ));
            }
            None
        });
        scanned_files += scanned;
        violations.append(&mut tree_violations);
    }

    assert!(
        scanned_files > 0,
        "Expected to scan production sources under scripts/ and src-tauri/src/"
    );
    assert!(
        violations.is_empty(),
        "Bundle público de Qobuz duplicado fuera de su sitio (escaneados {scanned_files} ficheros):\n{}",
        violations.join("\n")
    );
}

#[test]
fn test_qobuz_bridges_keep_env_override_contract() {
    let repo_root = get_repo_root();

    let qobuz_bridges = [
        repo_root.join("scripts").join("download_bridge.py"),
        repo_root.join("scripts").join("playlist_bridge.py"),
    ];

    for bridge in &qobuz_bridges {
        assert!(
            bridge.is_file(),
            "Production Qobuz bridge must exist: {:?}",
            bridge
        );
        let content = fs::read_to_string(bridge).expect("Read Qobuz bridge");

        // El override por entorno es la vía para credenciales propias del
        // operador; su ausencia NO es error: cae al bundle público.
        assert!(
            content.contains("QOBUZ_APP_ID") || content.contains("APP_ID"),
            "{:?} debe resolver el app_id (credenciales → env → público)",
            bridge
        );
        assert!(
            content.contains("os.getenv") || content.contains("QOBUZ_APP_SECRET"),
            "{:?} debe mantener la vía de override por entorno",
            bridge
        );
    }
}
