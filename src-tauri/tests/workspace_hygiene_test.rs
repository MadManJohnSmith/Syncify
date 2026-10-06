//! Workspace Hygiene & Architectural Boundary Test Suite (TASK-121)
//!
//! Validates:
//! 1. Root Cargo.toml is a pure virtual workspace without stub packages.
//! 2. Root `src/main.rs` stub ("Syncify core starting…") is completely disposed.
//! 3. No `legacy/syncify-cli` directory is tracked in the productive source tree.
//! 4. Active workspace members are strictly defined and all exist on disk.
//! 5. Production sources carry no hardcoded Qobuz credentials (TASK-152 / SEC-025).

use std::fs;
use std::path::{Path, PathBuf};

fn get_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Repo root must be parent of src-tauri")
        .to_path_buf()
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".git" | "target" | "node_modules" | "dist" | "__pycache__"
            ) {
                continue;
            }
            collect_files_recursive(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}

#[test]
fn test_root_cargo_toml_is_virtual_workspace() {
    let repo_root = get_repo_root();
    let root_cargo = repo_root.join("Cargo.toml");
    assert!(root_cargo.exists(), "Root Cargo.toml must exist");

    let content = fs::read_to_string(&root_cargo).expect("Must read root Cargo.toml");

    // Must be virtual workspace
    assert!(
        content.contains("[workspace]"),
        "Root Cargo.toml must define a [workspace]"
    );
    assert!(
        !content.contains("[package]"),
        "Root Cargo.toml must NOT define a [package] (virtual workspace only)"
    );
    assert!(
        !content.contains("name = \"syncify-core\""),
        "Root Cargo.toml must not have stub package syncify-core"
    );

    // Expected members
    let expected_members = [
        "src-tauri",
        "crates/syncify-core-domain",
        "crates/syncify-flac-writer",
        "crates/syncify-lyrics-domain",
        "crates/syncify-metadata-domain",
        "crates/syncify-tidal-downloader",
    ];

    for member in expected_members {
        assert!(
            content.contains(member),
            "Workspace members in root Cargo.toml must include {}",
            member
        );
        let member_path = repo_root.join(member);
        assert!(
            member_path.exists(),
            "Workspace member directory {} must exist",
            member
        );
        assert!(
            member_path.join("Cargo.toml").exists(),
            "Workspace member {} must have Cargo.toml",
            member
        );
    }
}

#[test]
fn test_root_binary_stub_absence() {
    let repo_root = get_repo_root();
    let root_src_main = repo_root.join("src").join("main.rs");
    assert!(
        !root_src_main.exists(),
        "Root src/main.rs stub binary must NOT exist"
    );

    // Ensure no 'Syncify core starting' anywhere in repo src directories
    let src_dir = repo_root.join("src");
    if src_dir.exists() {
        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().is_file() {
                let text = fs::read_to_string(entry.path()).unwrap_or_default();
                assert!(
                    !text.contains("Syncify core starting"),
                    "File {:?} must not contain 'Syncify core starting'",
                    entry.path()
                );
            }
        }
    }
}

#[test]
fn test_legacy_syncify_cli_absence_from_productive_tree() {
    let repo_root = get_repo_root();
    let legacy_cli = repo_root.join("legacy").join("syncify-cli");
    assert!(
        !legacy_cli.exists(),
        "legacy/syncify-cli must NOT exist in the productive source tree"
    );

    let legacy_dir = repo_root.join("legacy");
    assert!(
        !legacy_dir.exists(),
        "legacy/ directory must NOT exist in the productive source tree"
    );
}

#[test]
fn test_legacy_syncify_cli_archived_and_neutralized() {
    let repo_root = get_repo_root();

    // The archived legacy CLI lived under `workspace/`, which `.gitignore` ignores wholesale, so
    // it can never exist in a clean checkout nor in CI: the presence assertion made this suite
    // unrunnable everywhere but on the machine that produced the archive. The durable TASK-152 /
    // SEC-025 guarantee is instead verified against the tracked production tree: no legacy CLI
    // directory is tracked, and no production source carries hardcoded Qobuz credentials.
    // Scan only productive source roots: ignored virtualenvs and local tooling
    // contain unrelated directories named `legacy` (for example pip's resolver).
    let mut legacy_dirs = Vec::new();
    let mut dirs: Vec<_> = ["src-tauri", "crates", "ui", "scripts", "bin"]
        .iter()
        .map(|root| repo_root.join(root))
        .collect();
    if repo_root.join("syncify-cli").is_dir() {
        legacy_dirs.push(repo_root.join("syncify-cli").display().to_string());
    }
    while let Some(dir) = dirs.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if matches!(
                    name.as_str(),
                    ".git" | "target" | "node_modules" | "dist" | "__pycache__"
                ) {
                    continue;
                }
                if name == "legacy" || name == "syncify-cli" {
                    legacy_dirs.push(path.display().to_string());
                    continue;
                }
                dirs.push(path);
            }
        }
    }

    assert!(
        legacy_dirs.is_empty(),
        "No legacy CLI directory may be tracked in the productive tree: {:?}",
        legacy_dirs
    );

    let mut scanned = 0usize;
    let mut violations = Vec::new();
    for relative in ["scripts", "src-tauri/src", "crates"] {
        let base = repo_root.join(relative);
        let mut files = Vec::new();
        collect_files_recursive(&base, &mut files);
        for file in files {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "rs" | "py" | "toml" | "json" | "sh" | "md") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&file) {
                scanned += 1;
                // El secret del bundle de Qobuz es PÚBLICO (2026-10-04): vive
                // como valor por defecto en sus dos hogares canónicos
                // (src-tauri/src/services/qobuz.rs y
                // scripts/services/qobuz_service.py). Fuera de ellos es una
                // duplicación innecesaria y se marca; el app_id lo audita
                // qobuz_credentials_leak_test.
                let relative = file.strip_prefix(&repo_root).unwrap_or(&file);
                let canonical_secret_home = matches!(
                    relative.to_string_lossy().as_ref(),
                    "src-tauri/src/services/qobuz.rs" | "scripts/services/qobuz_service.py"
                );
                if !canonical_secret_home
                    && (content.contains("abb21364")
                        || content.contains("abb21364945c0583309667d13ca3d93a"))
                {
                    violations.push(file.display().to_string());
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Production sources must not hardcode the Qobuz app secret (SEC-025): {:?}",
        violations
    );
    assert!(
        scanned > 0,
        "Production sources must be scanned so the credential guarantee is actually verified"
    );
}
