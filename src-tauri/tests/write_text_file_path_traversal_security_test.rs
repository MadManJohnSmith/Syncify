//! Security Test Suite for [TASK-87] / [SEC-003]
//! Path Traversal Mitigation and Sandbox Confinement in `write_text_file` IPC Command
//!
//! Validates:
//! 1. Absolute path enforcement & rejection of path traversal (`..`) sequences to /etc/, ~/.bashrc, ~/.ssh/.
//! 2. Confinement to allowed user directories (Downloads, Documents, app data).
//! 3. Extension whitelisting (.txt, .json, .csv, .m3u, .m3u8, .log, .lrc, .ttml)
//!    and rejection of dangerous extensions (.sh, .exe, .bat, .bashrc, .desktop, .service).
//! 4. Prevention of symlink hijacking / traversal.
//! 5. Successful writes for legitimate paths and formats with cleanup in allowed directories.

use std::fs;
use std::path::PathBuf;
use syncify_tauri_lib::commands::{
    get_allowed_write_directories, validate_safe_write_path, validate_safe_write_path_with_bases,
    write_text_file, ALLOWED_WRITE_EXTENSIONS,
};

/// Resolves a writable directory that is strictly confined within Documents for live tests.
fn resolve_writable_documents_test_dir() -> PathBuf {
    let doc_dir = dirs::document_dir().expect("Documents directory must be resolvable");
    let candidate = if doc_dir.join("Syncify/target").exists() {
        doc_dir.join("Syncify/target/sec003_e2e_tests")
    } else {
        doc_dir.join("syncify_sec003_e2e_tests")
    };
    let _ = fs::create_dir_all(&candidate);
    candidate
}

#[tokio::test]
async fn test_path_traversal_sequences_rejected() {
    let download_dir = dirs::download_dir().expect("Downloads directory must be resolvable");
    let doc_dir = dirs::document_dir().expect("Documents directory must be resolvable");

    // Traversal using .. components toward sensitive targets (/etc/, ~/.bashrc, ~/.ssh/)
    let traversal_cases = [
        download_dir.join("../.bashrc").to_string_lossy().to_string(),
        download_dir.join("../../etc/passwd.txt").to_string_lossy().to_string(),
        download_dir.join("../.ssh/authorized_keys").to_string_lossy().to_string(),
        download_dir.join("sub/../../.profile").to_string_lossy().to_string(),
        doc_dir.join("../../../etc/shadow.log").to_string_lossy().to_string(),
        doc_dir.join("../.bashrc").to_string_lossy().to_string(),
        doc_dir.join("../.ssh/id_rsa.txt").to_string_lossy().to_string(),
        "/etc/passwd".to_string(),
        "/etc/cron.d/malicious.txt".to_string(),
        "/var/log/audit.log".to_string(),
    ];

    for path in traversal_cases {
        let result = write_text_file(path.clone(), "malicious_payload".to_string()).await;
        assert!(
            result.is_err(),
            "Path traversal attempt must be rejected for path: {}",
            path
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Error message for {} must indicate sandbox violation or access denied, got: {}",
            path,
            err
        );
    }
}

#[tokio::test]
async fn test_relative_paths_rejected() {
    let relative_cases = [
        "export.txt".to_string(),
        "./export.json".to_string(),
        "../export.csv".to_string(),
        "subdir/export.m3u".to_string(),
    ];

    for path in relative_cases {
        let result = write_text_file(path.clone(), "some text".to_string()).await;
        assert!(
            result.is_err(),
            "Relative path must be rejected: {}",
            path
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("absoluta"),
            "Error should mention absolute path requirement for {}, got: {}",
            path,
            err
        );
    }
}

#[tokio::test]
async fn test_dangerous_extensions_rejected_in_allowed_directories() {
    let download_dir = dirs::download_dir().expect("Downloads directory must be resolvable");
    let doc_dir = dirs::document_dir().expect("Documents directory must be resolvable");

    let dangerous_files = [
        download_dir.join("malware.sh"),
        download_dir.join("payload.exe"),
        download_dir.join("script.bat"),
        download_dir.join("app.desktop"),
        download_dir.join("daemon.service"),
        download_dir.join(".bashrc"),
        download_dir.join(".bash_profile"),
        download_dir.join(".profile"),
        download_dir.join("exploit.py"),
        download_dir.join("binary.bin"),
        download_dir.join("library.so"),
        download_dir.join("library.dll"),
        doc_dir.join("evil.sh"),
        doc_dir.join("ransomware.exe"),
        doc_dir.join("startup.service"),
        doc_dir.join(".hidden_config"),
    ];

    for path in dangerous_files {
        let path_str = path.to_string_lossy().to_string();
        let result = write_text_file(path_str.clone(), "malicious code".to_string()).await;
        assert!(
            result.is_err(),
            "Dangerous extension/file must be rejected: {}",
            path_str
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("extensión") || err.contains("ocultos") || err.contains("sandbox violation"),
            "Error must identify forbidden extension or hidden file for {}, got: {}",
            path_str,
            err
        );
    }
}

#[tokio::test]
async fn test_empty_path_or_content_rejected() {
    assert!(
        write_text_file("".to_string(), "hello".to_string()).await.is_err(),
        "Empty path must be rejected"
    );
    assert!(
        write_text_file("   ".to_string(), "hello".to_string()).await.is_err(),
        "Whitespace-only path must be rejected"
    );

    let doc_dir = dirs::document_dir().expect("Documents directory must be resolvable");
    let valid_path = doc_dir.join("empty_test.txt").to_string_lossy().to_string();
    let res_empty_content = write_text_file(valid_path, "".to_string()).await;
    assert!(
        res_empty_content.is_err(),
        "Empty content must be rejected"
    );
}

#[tokio::test]
async fn test_legitimate_writes_in_documents_and_downloads() {
    let base_test_dir = resolve_writable_documents_test_dir();
    let test_dir = base_test_dir.join("e2e_legitimate_writes");
    let _ = fs::create_dir_all(&test_dir);
    let doc_dir = dirs::document_dir().expect("Documents directory must be resolvable");
    let download_dir = dirs::download_dir().expect("Downloads directory must be resolvable");

    // Verify test_dir is strictly inside Document directory
    assert!(
        test_dir.starts_with(&doc_dir),
        "test_dir must be inside Document directory"
    );

    // 1. Full E2E write execution in Documents for allowed formats: .txt, .json, .m3u, .lrc, .csv
    let test_cases = [
        (
            test_dir.join("syncify_sec003_test_export.txt"),
            "Syncify text export test payload",
        ),
        (
            test_dir.join("syncify_sec003_test_metadata.json"),
            r#"{"title": "Test Track", "artist": "Test Artist"}"#,
        ),
        (
            test_dir.join("syncify_sec003_test_playlist.m3u"),
            "#EXTM3U\n#EXTINF:180,Artist - Title\nTrack.flac\n",
        ),
        (
            test_dir.join("syncify_sec003_test_lyrics.lrc"),
            "[00:01.00]Line 1\n[00:05.00]Line 2\n",
        ),
        (
            test_dir.join("syncify_sec003_test_data.csv"),
            "id,track,artist\n1,Track1,Artist1\n",
        ),
    ];

    for (target_path, content) in test_cases {
        let path_str = target_path.to_string_lossy().to_string();
        let expected_bytes = content.as_bytes().len() as u64;

        let result = write_text_file(path_str.clone(), content.to_string()).await;
        assert!(
            result.is_ok(),
            "Legitimate write failed for {}: {:?}",
            path_str,
            result.err()
        );
        let written = result.unwrap();
        assert_eq!(written, expected_bytes, "Written byte count must match");

        // Verify content on disk
        let read_back = fs::read_to_string(&target_path).expect("File must exist and be readable");
        assert_eq!(read_back, content, "Content written to disk must match exactly");

        // Clean up test file
        let _ = fs::remove_file(&target_path);
    }

    // 2. Validate Downloads path validation for legitimate exports
    let legitimate_download_paths = [
        download_dir.join("lyrics_export.lrc"),
        download_dir.join("tracklist.m3u8"),
        download_dir.join("metadata_dump.json"),
        download_dir.join("summary.txt"),
    ];

    for p in &legitimate_download_paths {
        let res = validate_safe_write_path(p);
        assert!(
            res.is_ok(),
            "Validation for legitimate Downloads path {:?} should succeed: {:?}",
            p,
            res.err()
        );
    }

    // Clean up test directory
    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_nested_subdirectory_creation_in_allowed_directory() {
    let base_test_dir = resolve_writable_documents_test_dir();
    let test_dir = base_test_dir.join("nested_creation_test");
    let nested_dir = test_dir.join("nested_folder");
    let target_file = nested_dir.join("subfolder").join("test_log.log");
    let content = "2026-03-30 12:00:00 [INFO] Syncify test log entry\n";

    let result = write_text_file(
        target_file.to_string_lossy().to_string(),
        content.to_string(),
    )
    .await;

    assert!(
        result.is_ok(),
        "Nested directory creation within allowed directory should succeed: {:?}",
        result.err()
    );

    let read_back = fs::read_to_string(&target_file).expect("File must be readable");
    assert_eq!(read_back, content);

    // Clean up created test folder
    let _ = fs::remove_file(&target_file);
    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_symlink_overwriting_prevented() {
    let temp_sandbox = tempfile::tempdir().expect("Must create temp sandbox dir");
    let outside_dir = tempfile::tempdir().expect("Must create outside dir");

    let allowed_bases = vec![temp_sandbox.path().to_path_buf()];

    // Create a target file outside the sandbox
    let outside_target = outside_dir.path().join("victim.txt");
    fs::write(&outside_target, "pre-existing victim content").expect("Write outside victim");

    // Create a symlink inside the sandbox pointing to the outside victim file
    let symlink_path = temp_sandbox.path().join("link_to_victim.txt");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_target, &symlink_path).expect("Create symlink");

    #[cfg(unix)]
    {
        let validation_res = validate_safe_write_path_with_bases(&symlink_path, &allowed_bases);
        assert!(
            validation_res.is_err(),
            "Writing to a symlink must be rejected"
        );
        let err = validation_res.unwrap_err();
        assert!(
            err.contains("enlaces simbólicos") || err.contains("sandbox violation"),
            "Error message must specify symlink protection, got: {}",
            err
        );

        // Verify victim file was not modified
        let victim_content = fs::read_to_string(&outside_target).expect("Read victim");
        assert_eq!(victim_content, "pre-existing victim content");
    }
}

#[tokio::test]
async fn test_all_whitelisted_extensions_allowed() {
    let temp_sandbox = tempfile::tempdir().expect("Must create temp sandbox dir");
    let allowed_bases = vec![temp_sandbox.path().to_path_buf()];

    for ext in ALLOWED_WRITE_EXTENSIONS {
        let file_path = temp_sandbox.path().join(format!("test_ext.{}", ext));
        let res = validate_safe_write_path_with_bases(&file_path, &allowed_bases);
        assert!(
            res.is_ok(),
            "Extension .{} must be allowed, got error: {:?}",
            ext,
            res.err()
        );
    }
}

#[test]
fn test_allowed_write_directories_are_populated() {
    let bases = get_allowed_write_directories();
    assert!(
        !bases.is_empty(),
        "get_allowed_write_directories must discover at least one allowed directory"
    );
    for base in &bases {
        assert!(
            base.is_absolute(),
            "All allowed base directories must be absolute paths: {:?}",
            base
        );
    }
}
