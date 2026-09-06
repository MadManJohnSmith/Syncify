//! Regression and Security Tests for TASK-114 / SEC-018
//!
//! Validates:
//! 1. Abandoned staging files within legitimate staging root are successfully purged.
//! 2. Path traversal attempts with parent components (`..`) are rejected and external files remain untouched.
//! 3. Arbitrary absolute paths outside staging root are rejected and external files remain untouched.
//! 4. Empty or whitespace-only paths are rejected with structured error.
//! 5. Directory paths (including staging root itself) cannot be removed.
//! 6. Symlink traversal pointing outside staging directory is detected and rejected.

use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::path::Path;
use syncify_tauri_lib::commands::perform_repair_integrity_issues;
use tempfile::TempDir;

async fn setup_test_db(base_folder: &Path) -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    let base_folder_str = base_folder.to_string_lossy().to_string();

    sqlx::query("UPDATE folder_settings SET base_folder = ? WHERE id = 1")
        .bind(&base_folder_str)
        .execute(&pool)
        .await
        .expect("Failed to update base_folder");

    pool
}

#[tokio::test]
async fn test_staging_purge_legitimate_file_succeeds() {
    let temp_root = TempDir::new().expect("Failed to create temp root");
    let base_dir = temp_root.path().join("music");
    fs::create_dir_all(&base_dir).unwrap();

    let staging_dir = base_dir.join(".staging");
    fs::create_dir_all(&staging_dir).unwrap();

    let valid_part = staging_dir.join("track_12345.flac.part");
    fs::write(&valid_part, b"PARTIAL_FLAC_PAYLOAD").unwrap();
    assert!(valid_part.exists(), "Test file must exist before purge");

    let pool = setup_test_db(&base_dir).await;

    let res = perform_repair_integrity_issues(
        &pool,
        Some(vec![valid_part.to_string_lossy().to_string()]),
    )
    .await
    .expect("Legitimate staging purge must succeed");

    assert_eq!(res.purged_staging_files, 1);
    assert!(!valid_part.exists(), "Staged file must be removed from disk");
}

#[tokio::test]
async fn test_staging_purge_rejects_parent_directory_traversal() {
    let temp_root = TempDir::new().expect("Failed to create temp root");
    let base_dir = temp_root.path().join("music");
    fs::create_dir_all(&base_dir).unwrap();

    let staging_dir = base_dir.join(".staging");
    fs::create_dir_all(&staging_dir).unwrap();

    // Sensitive external file outside staging (in parent base_dir)
    let outside_file = base_dir.join("library_database_do_not_delete.txt");
    fs::write(&outside_file, b"CRITICAL_DATA").unwrap();
    assert!(outside_file.exists());

    let pool = setup_test_db(&base_dir).await;

    // Craft path traversal with ..
    let traversal_path = staging_dir
        .join("..")
        .join("library_database_do_not_delete.txt")
        .to_string_lossy()
        .to_string();

    let err = perform_repair_integrity_issues(&pool, Some(vec![traversal_path]))
        .await
        .expect_err("Traversal path with .. must be rejected");

    assert!(
        err.contains("Path traversal attempt detected"),
        "Error message must indicate traversal detection: {}",
        err
    );
    assert!(
        outside_file.exists(),
        "External file outside staging must remain intact on disk"
    );
}

#[tokio::test]
async fn test_staging_purge_rejects_arbitrary_outside_path() {
    let temp_root = TempDir::new().expect("Failed to create temp root");
    let base_dir = temp_root.path().join("music");
    fs::create_dir_all(&base_dir).unwrap();

    // Sensitive external file in a completely separate directory
    let outside_dir = TempDir::new().expect("Failed to create outside dir");
    let sensitive_file = outside_dir.path().join("id_rsa_secret.pem");
    fs::write(&sensitive_file, b"SUPER_SECRET_KEY").unwrap();
    assert!(sensitive_file.exists());

    let pool = setup_test_db(&base_dir).await;

    let err = perform_repair_integrity_issues(
        &pool,
        Some(vec![sensitive_file.to_string_lossy().to_string()]),
    )
    .await
    .expect_err("Arbitrary path outside staging must be rejected");

    assert!(
        err.contains("Path traversal attempt detected"),
        "Error message must indicate traversal detection: {}",
        err
    );
    assert!(
        sensitive_file.exists(),
        "External sensitive file must remain completely untouched on disk"
    );
}

#[tokio::test]
async fn test_staging_purge_rejects_empty_paths() {
    let temp_root = TempDir::new().expect("Failed to create temp root");
    let base_dir = temp_root.path().join("music");
    fs::create_dir_all(&base_dir).unwrap();

    let pool = setup_test_db(&base_dir).await;

    let err_empty = perform_repair_integrity_issues(&pool, Some(vec!["".to_string()]))
        .await
        .expect_err("Empty string path must be rejected");
    assert!(err_empty.contains("Path traversal attempt detected"));

    let err_whitespace = perform_repair_integrity_issues(&pool, Some(vec!["   \t \n".to_string()]))
        .await
        .expect_err("Whitespace-only path must be rejected");
    assert!(err_whitespace.contains("Path traversal attempt detected"));
}

#[tokio::test]
async fn test_staging_purge_cannot_delete_staging_directory_itself() {
    let temp_root = TempDir::new().expect("Failed to create temp root");
    let base_dir = temp_root.path().join("music");
    fs::create_dir_all(&base_dir).unwrap();

    let staging_dir = base_dir.join(".staging");
    fs::create_dir_all(&staging_dir).unwrap();
    assert!(staging_dir.exists());

    let pool = setup_test_db(&base_dir).await;

    let err = perform_repair_integrity_issues(
        &pool,
        Some(vec![staging_dir.to_string_lossy().to_string()]),
    )
    .await
    .expect_err("Attempt to purge staging directory itself must be rejected");

    assert!(
        err.contains("Path traversal attempt detected"),
        "Error message must indicate rejection: {}",
        err
    );
    assert!(
        staging_dir.exists(),
        "Staging directory itself must NOT be deleted"
    );
}

#[tokio::test]
async fn test_staging_purge_nested_subfolder_file_allowed() {
    let temp_root = TempDir::new().expect("Failed to create temp root");
    let base_dir = temp_root.path().join("music");
    fs::create_dir_all(&base_dir).unwrap();

    let staging_nested = base_dir.join(".staging").join("session_123");
    fs::create_dir_all(&staging_nested).unwrap();

    let nested_part = staging_nested.join("track_sub.part");
    fs::write(&nested_part, b"NESTED_PARTIAL").unwrap();
    assert!(nested_part.exists());

    let pool = setup_test_db(&base_dir).await;

    let res = perform_repair_integrity_issues(
        &pool,
        Some(vec![nested_part.to_string_lossy().to_string()]),
    )
    .await
    .expect("Legitimate nested file inside staging must be purged");

    assert_eq!(res.purged_staging_files, 1);
    assert!(!nested_part.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn test_staging_purge_rejects_symlink_pointing_outside_staging() {
    use std::os::unix::fs::symlink;

    let temp_root = TempDir::new().expect("Failed to create temp root");
    let base_dir = temp_root.path().join("music");
    fs::create_dir_all(&base_dir).unwrap();

    let staging_dir = base_dir.join(".staging");
    fs::create_dir_all(&staging_dir).unwrap();

    let outside_target = temp_root.path().join("important_system_file.txt");
    fs::write(&outside_target, b"SYSTEM_CORE_RECORD").unwrap();

    let evil_symlink = staging_dir.join("fake_part.flac");
    symlink(&outside_target, &evil_symlink).expect("Create symlink");

    let pool = setup_test_db(&base_dir).await;

    let err = perform_repair_integrity_issues(
        &pool,
        Some(vec![evil_symlink.to_string_lossy().to_string()]),
    )
    .await
    .expect_err("Symlink pointing outside staging must be rejected");

    assert!(
        err.contains("Path traversal attempt detected"),
        "Error message must indicate rejection: {}",
        err
    );
    assert!(
        outside_target.exists(),
        "Symlink target outside staging must remain intact"
    );
}
