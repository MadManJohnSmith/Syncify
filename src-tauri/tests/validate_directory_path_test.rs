use std::fs;
use syncify_tauri_lib::commands::validate_directory_path;
use tempfile::TempDir;

#[tokio::test]
async fn test_validate_nonexistent_directory_does_not_create_path_or_probe() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nested_path = temp_dir
        .path()
        .join("syncify_nonexistent_test_dir_123")
        .join("sub_nested");

    // Pre-condition: neither directory exists
    assert!(!nested_path.exists(), "Target nested path must not exist prior to validation");
    assert!(
        !nested_path.parent().unwrap().exists(),
        "Parent component must not exist prior to validation"
    );

    let res = validate_directory_path(nested_path.to_string_lossy().to_string())
        .await
        .expect("validate_directory_path should succeed");

    // Post-condition: directories must NOT have been created on disk
    assert!(
        !nested_path.exists(),
        "validate_directory_path must NOT create the requested directory"
    );
    assert!(
        !nested_path.parent().unwrap().exists(),
        "validate_directory_path must NOT create intermediate ancestor directories"
    );

    // Validation result assertions
    assert!(res.valid, "Path should be considered valid since existing ancestor is writable");
    assert!(!res.exists, "res.exists must be false for nonexistent path");
    assert!(!res.is_dir, "res.is_dir must be false for nonexistent path");
    assert!(res.is_writable, "res.is_writable must be true based on existing ancestor check");
    assert!(res.drive_mounted, "Drive must be mounted");
    assert!(res.available_bytes > 0, "Drive must report available bytes");
    assert!(res.error_message.is_none(), "Error message must be None: {:?}", res.error_message);

    // Ensure no probe files were left behind in the existing ancestor
    let temp_entries: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read temp dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".syncify_probe_"))
        .collect();
    assert!(
        temp_entries.is_empty(),
        "Probe files must be cleaned up immediately and not left behind: {:?}",
        temp_entries
    );
}

#[tokio::test]
async fn test_validate_existing_directory_behavior() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let existing_sub = temp_dir.path().join("existing_folder");
    fs::create_dir_all(&existing_sub).expect("Failed to create test folder");

    let res = validate_directory_path(existing_sub.to_string_lossy().to_string())
        .await
        .expect("validate_directory_path should succeed");

    assert!(res.valid);
    assert!(res.exists);
    assert!(res.is_dir);
    assert!(res.is_writable);
    assert!(res.drive_mounted);
    assert!(res.error_message.is_none());

    let entries: Vec<_> = fs::read_dir(&existing_sub)
        .expect("Failed to read existing folder")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".syncify_probe_"))
        .collect();
    assert!(entries.is_empty(), "Probe files must be cleaned up in existing directory");
}

#[tokio::test]
async fn test_validate_existing_file_fails_is_dir() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("some_file.txt");
    fs::write(&file_path, b"hello").expect("Failed to write file");

    let res = validate_directory_path(file_path.to_string_lossy().to_string())
        .await
        .expect("validate_directory_path should succeed");

    assert!(!res.valid);
    assert!(res.exists);
    assert!(!res.is_dir);
    assert!(!res.is_writable);
    assert!(res.error_message.is_some());
}

#[tokio::test]
async fn test_validate_empty_path_fails() {
    let res = validate_directory_path("   ".to_string())
        .await
        .expect("validate_directory_path should handle empty path");

    assert!(!res.valid);
    assert!(!res.exists);
    assert!(!res.is_dir);
    assert!(!res.is_writable);
    assert!(res.error_message.is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn test_validate_nonexistent_directory_under_unwritable_ancestor() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let readonly_parent = temp_dir.path().join("readonly_dir");
    fs::create_dir(&readonly_parent).expect("Failed to create readonly dir");

    // Make parent read-only (0o555: r-x r-x r-x)
    let ro_perms = fs::Permissions::from_mode(0o555);
    fs::set_permissions(&readonly_parent, ro_perms).expect("Failed to set read-only permissions");

    let nonexistent_target = readonly_parent.join("sub_dir");

    let res = validate_directory_path(nonexistent_target.to_string_lossy().to_string())
        .await
        .expect("validate_directory_path should return Result::Ok");

    // Target must NOT have been created
    assert!(!nonexistent_target.exists());

    // Non-root check: writable should be false, valid should be false
    if nix_is_non_root() {
        assert!(!res.valid, "Nonexistent target under unwritable ancestor must not be valid");
        assert!(!res.is_writable, "Must report not writable");
        assert!(res.error_message.is_some(), "Must report permission error");
    }

    // Restore permissions so TempDir cleanup works
    let rw_perms = fs::Permissions::from_mode(0o755);
    let _ = fs::set_permissions(&readonly_parent, rw_perms);
}

#[cfg(unix)]
fn nix_is_non_root() -> bool {
    extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() != 0 }
}
