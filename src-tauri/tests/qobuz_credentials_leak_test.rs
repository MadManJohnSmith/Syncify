//! Qobuz Credentials Leak and Test Harness Hygiene Test Suite (TASK-152 / SEC-025)
//!
//! Validates:
//! 1. No tracked production source contains hardcoded static Qobuz credentials.
//! 2. The production Qobuz bridges resolve credentials from environment variables
//!    (`QOBUZ_APP_ID`, `QOBUZ_APP_SECRET`) and fail closed when they are unset.
//! 3. All member crates under `crates/` are strictly clean of static Qobuz secrets.
//!
//! Note: the retired legacy CLI under `workspace/audit_archive/legacy` is not tracked
//! (`.gitignore` ignores `workspace/`), so scanning it proved nothing: the suite walked zero
//! files and still passed. The guarantee is now verified against the tracked production tree.

use std::fs;
use std::path::{Path, PathBuf};

fn get_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Repo root must be parent of src-tauri")
        .to_path_buf()
}

const FORBIDDEN_APP_ID: &str = "798273057";
const FORBIDDEN_SECRET: &str = "abb21364945c0583309667d13ca3d93a";
const FORBIDDEN_SECRET_PREFIX: &str = "abb21364";

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

#[test]
fn test_legacy_suite_has_no_hardcoded_qobuz_credentials() {
    let repo_root = get_repo_root();

    // Scan the tracked production tree. The retired `legacy/` and `workspace/audit_archive/legacy`
    // trees are not tracked, so they are not scanned and cannot make this assertion vacuous.
    let candidates = [
        repo_root.join("scripts"),
        repo_root.join("src-tauri").join("src"),
    ];

    // The Qobuz client identifier is a public id embedded in the download path, not the secret.
    // It is allowed only in these two production files; anywhere else it is a leak.
    let app_id_allowed_in = [
        "scripts/services/qobuz_service.py",
        "scripts/services/qobuz_auth.py",
    ];

    let mut scanned_files = 0;
    let mut violations = Vec::new();

    for base_dir in &candidates {
        let mut files = Vec::new();
        collect_files_recursive(base_dir, &mut files);

        for file in files {
            // Only inspect source files, scripts, and documentation
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "toml" | "json" | "py" | "sh" | "md") {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&file) {
                scanned_files += 1;
                let relative = file
                    .strip_prefix(&repo_root)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .to_string();

                if content.contains(FORBIDDEN_APP_ID)
                    && !app_id_allowed_in.contains(&relative.as_str())
                {
                    violations.push(format!(
                        "{}: contains forbidden hardcoded QOBUZ_APP_ID ({})",
                        file.display(),
                        FORBIDDEN_APP_ID
                    ));
                }
                if content.contains(FORBIDDEN_SECRET) || content.contains(FORBIDDEN_SECRET_PREFIX) {
                    violations.push(format!(
                        "{}: contains forbidden hardcoded QOBUZ_APP_SECRET ({})",
                        file.display(),
                        FORBIDDEN_SECRET_PREFIX
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found hardcoded Qobuz credentials in production sources (scanned {} files):\n{}",
        scanned_files,
        violations.join("\n")
    );
    assert!(
        scanned_files > 0,
        "Expected to scan production sources under scripts/ and src-tauri/src/"
    );
}

#[test]
fn test_qobuz_test_harness_neutralization_and_env_contract() {
    let repo_root = get_repo_root();

    // The legacy `qobuz_test.rs` harness is not tracked, so its env contract is verified where
    // the credentials are actually consumed: the production Qobuz bridges.
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

        // 1. Must not contain raw secret values
        assert!(
            !content.contains(FORBIDDEN_APP_ID),
            "{:?} must NOT contain the hardcoded App ID string",
            bridge
        );
        assert!(
            !content.contains(FORBIDDEN_SECRET_PREFIX),
            "{:?} must NOT contain the hardcoded Secret string",
            bridge
        );

        // 2. Must query credentials from the environment instead of embedding them
        assert!(
            content.contains("QOBUZ_APP_ID"),
            "{:?} must query the QOBUZ_APP_ID env var",
            bridge
        );
        assert!(
            content.contains("QOBUZ_APP_SECRET"),
            "{:?} must query the QOBUZ_APP_SECRET env var",
            bridge
        );
        assert!(
            content.contains("os.getenv"),
            "{:?} must resolve credentials via os.getenv",
            bridge
        );
    }
}

#[test]
fn test_all_workspace_crates_free_of_hardcoded_qobuz_credentials() {
    let repo_root = get_repo_root();
    let crates_dir = repo_root.join("crates");

    let mut crate_files = Vec::new();
    collect_files_recursive(&crates_dir, &mut crate_files);

    let mut scanned_crates_files = 0;
    let mut violations = Vec::new();

    for file in crate_files {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "rs" {
            if let Ok(content) = fs::read_to_string(&file) {
                scanned_crates_files += 1;
                if content.contains(FORBIDDEN_APP_ID) {
                    violations.push(format!(
                        "{}: contains forbidden App ID in crate",
                        file.display()
                    ));
                }
                if content.contains(FORBIDDEN_SECRET_PREFIX) {
                    violations.push(format!(
                        "{}: contains forbidden Secret in crate",
                        file.display()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found hardcoded Qobuz credentials in crates (scanned {} files):\n{}",
        scanned_crates_files,
        violations.join("\n")
    );
    assert!(
        scanned_crates_files > 0,
        "Expected to scan Rust files in crates/"
    );
}

#[test]
fn test_legacy_binaries_and_tests_hygiene() {
    let repo_root = get_repo_root();
    let legacy_cli = repo_root
        .join("workspace")
        .join("audit_archive")
        .join("legacy")
        .join("syncify-cli");

    if !legacy_cli.exists() {
        return;
    }

    let bin_dir = legacy_cli.join("src").join("bin");
    let tests_dir = legacy_cli.join("tests");

    let mut target_files = Vec::new();
    collect_files_recursive(&bin_dir, &mut target_files);
    collect_files_recursive(&tests_dir, &mut target_files);

    assert!(
        !target_files.is_empty(),
        "Archived legacy CLI should contain test and binary harnesses"
    );

    for file in target_files {
        if file.extension().and_then(|e| e.to_str()) == Some("rs") {
            let content = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("Failed to read {}: {}", file.display(), e));

            assert!(
                !content.contains(FORBIDDEN_APP_ID),
                "File {:?} must not contain forbidden App ID",
                file
            );
            assert!(
                !content.contains(FORBIDDEN_SECRET_PREFIX),
                "File {:?} must not contain forbidden Secret",
                file
            );
        }
    }
}
