use std::path::PathBuf;
use std::process::Command;

fn get_repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.join(".git").exists() {
        manifest_dir
    } else if let Some(parent) = manifest_dir.parent() {
        if parent.join(".git").exists() {
            parent.to_path_buf()
        } else {
            manifest_dir
        }
    } else {
        manifest_dir
    }
}

#[test]
fn test_gitignore_contains_bundle_and_backup_rules() {
    let repo_root = get_repo_root();
    let gitignore_path = repo_root.join(".gitignore");
    assert!(
        gitignore_path.exists(),
        ".gitignore must exist in repository root: {:?}",
        gitignore_path
    );

    let content = std::fs::read_to_string(&gitignore_path)
        .expect("Failed to read .gitignore");

    let lines: Vec<&str> = content.lines().map(|l| l.trim()).collect();

    // 1. Assert git bundle exclusion rules
    assert!(
        lines.contains(&"*.bundle"),
        ".gitignore must contain '*.bundle' rule"
    );
    assert!(
        lines.contains(&".*.bundle"),
        ".gitignore must contain '.*.bundle' rule to match hidden bundles"
    );

    // 2. Assert database backup exclusion rules
    assert!(
        lines.contains(&"*.db.backup"),
        ".gitignore must contain '*.db.backup' rule"
    );
    assert!(
        lines.contains(&"syncify_backup_*.db"),
        ".gitignore must contain 'syncify_backup_*.db' rule"
    );
}

#[test]
fn test_no_bundle_files_in_repository_root() {
    let repo_root = get_repo_root();
    let entries = std::fs::read_dir(&repo_root)
        .expect("Failed to read repo root directory");

    let mut residual_bundles = Vec::new();

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.ends_with(".bundle") {
            residual_bundles.push(name.into_owned());
        }
    }

    assert!(
        residual_bundles.is_empty(),
        "Found forbidden .bundle files in repo root: {:?}. These must be purged or moved out of the repo.",
        residual_bundles
    );
}

#[test]
fn test_git_ls_files_reports_no_tracked_bundles() {
    let repo_root = get_repo_root();

    let output = Command::new("git")
        .args(["ls-files", "*.bundle", ".*.bundle"])
        .current_dir(&repo_root)
        .output()
        .expect("Failed to execute git ls-files");

    assert!(
        output.status.success(),
        "git ls-files command failed: {:?}",
        output
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tracked_files: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();

    assert!(
        tracked_files.is_empty(),
        "Git repository must not track any .bundle files, but found: {:?}",
        tracked_files
    );
}

#[test]
fn test_git_check_ignore_simulates_bundle_and_backup_rejection() {
    let repo_root = get_repo_root();

    let test_cases = [
        ("dummy.bundle", "*.bundle"),
        (".dummy.bundle", ".*.bundle"),
        ("src-tauri/temp.bundle", "*.bundle"),
        ("dummy.db.backup", "*.db.backup"),
        ("syncify_backup_pre_repair_TASK-151.db", "syncify_backup_*.db"),
    ];

    for (target_path, expected_rule) in test_cases {
        let output = Command::new("git")
            .args(["check-ignore", "-v", target_path])
            .current_dir(&repo_root)
            .output()
            .expect("Failed to execute git check-ignore");

        assert!(
            output.status.success(),
            "git check-ignore should return success (ignored) for '{}', but got exit status {:?}: {}",
            target_path,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(".gitignore:"),
            "Expected ignore match from .gitignore for '{}', got stdout: {}",
            target_path,
            stdout
        );
        assert!(
            stdout.contains(expected_rule),
            "Expected matching rule '{}' in check-ignore output for '{}', got stdout: {}",
            expected_rule,
            target_path,
            stdout
        );
    }
}
