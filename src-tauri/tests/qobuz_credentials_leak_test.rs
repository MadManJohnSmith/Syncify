//! Qobuz Credentials Leak and Test Harness Hygiene Test Suite (TASK-152 / SEC-025)
//!
//! Validates:
//! 1. The legacy test harness `qobuz_test.rs` does NOT contain hardcoded static credentials.
//! 2. `qobuz_test.rs` safely resolves credentials from environment variables (`QOBUZ_APP_ID`, `QOBUZ_APP_SECRET`)
//!    with sanitized `<REDACTED_DEV_KEY>` placeholders.
//! 3. Neither `798273057` nor `abb21364945c0583309667d13ca3d93a` / `abb21364` exist anywhere in the legacy
//!    CLI suite (`legacy/syncify-cli` or `workspace/audit_archive/legacy/syncify-cli`).
//! 4. All member crates under `crates/` are strictly clean of static Qobuz secrets.

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
const REDACTED_PLACEHOLDER: &str = "<REDACTED_DEV_KEY>";

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

    // Check both potential root legacy/ directory and the canonical archive location
    let candidates = [
        repo_root.join("legacy"),
        repo_root.join("workspace").join("audit_archive").join("legacy"),
    ];

    let mut scanned_files = 0;
    let mut violations = Vec::new();

    for base_dir in &candidates {
        if !base_dir.exists() {
            continue;
        }

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
                if content.contains(FORBIDDEN_APP_ID) {
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
        "Found hardcoded Qobuz credentials in legacy files (scanned {} files):\n{}",
        scanned_files,
        violations.join("\n")
    );
    assert!(
        scanned_files > 0,
        "Expected to scan at least one legacy file under workspace/audit_archive/legacy"
    );
}

#[test]
fn test_qobuz_test_harness_neutralization_and_env_contract() {
    let repo_root = get_repo_root();
    let qobuz_test_paths = [
        repo_root
            .join("workspace")
            .join("audit_archive")
            .join("legacy")
            .join("syncify-cli")
            .join("src")
            .join("bin")
            .join("qobuz_test.rs"),
        repo_root
            .join("legacy")
            .join("syncify-cli")
            .join("src")
            .join("bin")
            .join("qobuz_test.rs"),
    ];

    let existing_path = qobuz_test_paths
        .iter()
        .find(|p| p.exists())
        .expect("At least one qobuz_test.rs must exist in archive or legacy");

    let content = fs::read_to_string(existing_path).expect("Read qobuz_test.rs");

    // 1. Must not contain raw secret values
    assert!(
        !content.contains(FORBIDDEN_APP_ID),
        "qobuz_test.rs must NOT contain the hardcoded App ID string"
    );
    assert!(
        !content.contains(FORBIDDEN_SECRET_PREFIX),
        "qobuz_test.rs must NOT contain the hardcoded Secret string"
    );

    // 2. Must contain sanitized placeholder
    assert!(
        content.contains(REDACTED_PLACEHOLDER),
        "qobuz_test.rs must use the sanitized <REDACTED_DEV_KEY> placeholder"
    );

    // 3. Must dynamically read from environment
    assert!(
        content.contains("QOBUZ_APP_ID"),
        "qobuz_test.rs must query the QOBUZ_APP_ID env var"
    );
    assert!(
        content.contains("QOBUZ_APP_SECRET"),
        "qobuz_test.rs must query the QOBUZ_APP_SECRET env var"
    );
    assert!(
        content.contains("std::env::var"),
        "qobuz_test.rs must resolve credentials via std::env::var"
    );
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
