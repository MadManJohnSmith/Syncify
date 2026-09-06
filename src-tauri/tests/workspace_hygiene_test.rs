//! Workspace Hygiene & Architectural Boundary Test Suite (TASK-121)
//!
//! Validates:
//! 1. Root Cargo.toml is a pure virtual workspace without stub packages.
//! 2. Root `src/main.rs` stub ("Syncify core starting…") is completely disposed.
//! 3. `legacy/syncify-cli` is removed from the productive source tree and safely archived.
//! 4. Active workspace members are strictly defined and all exist on disk.
//! 5. Archived legacy artifacts have credentials neutralized (TASK-152 unblock).

use std::fs;
use std::path::{Path, PathBuf};

fn get_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Repo root must be parent of src-tauri")
        .to_path_buf()
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
    let archive_cli = repo_root
        .join("workspace")
        .join("audit_archive")
        .join("legacy")
        .join("syncify-cli");

    assert!(
        archive_cli.exists(),
        "legacy/syncify-cli must be safely preserved under workspace/audit_archive/legacy/syncify-cli"
    );
    assert!(
        archive_cli.join("Cargo.toml").exists(),
        "Archived legacy/syncify-cli must contain its Cargo.toml"
    );

    // Verify credentials neutralization (TASK-152 criteria)
    let qobuz_test_path = archive_cli.join("src").join("bin").join("qobuz_test.rs");
    if qobuz_test_path.exists() {
        let content = fs::read_to_string(&qobuz_test_path).expect("Read qobuz_test.rs");
        assert!(
            !content.contains("798273057"),
            "Archived qobuz_test.rs must not contain hardcoded QOBUZ_APP_ID"
        );
        assert!(
            !content.contains("abb21364"),
            "Archived qobuz_test.rs must not contain hardcoded QOBUZ_APP_SECRET"
        );
        assert!(
            content.contains("<REDACTED_DEV_KEY>"),
            "Archived qobuz_test.rs must contain neutralized <REDACTED_DEV_KEY>"
        );
    }
}
