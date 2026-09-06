//! Test suite for Application Profile Permissions Hardening & LocalStorage Audit [TASK-112]
//!
//! Validates:
//! 1. Directory permissions are hardened to 0700 on Unix.
//! 2. Sensitive files (syncify.db, .crypto_key, cookies, locks, etc.) are hardened to 0600 on Unix.
//! 3. Residual Spotify OAuth WebView localstorage databases are audited and purged.
//! 4. Process umask (0o077) enforces secure default creation modes.
//! 5. Platform compatibility across Unix and non-Unix environments.

use std::fs::{self, File};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use syncify_tauri_lib::crypto::{
    audit_and_purge_webview_localstorage, ensure_secure_profile_permissions,
    set_secure_process_umask,
};

#[test]
fn test_ensure_secure_profile_permissions_hardens_directories_and_files() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let profile_path = temp_dir.path();

    // Create subdirectories with permissive permissions (0755 / 0777)
    let subdirs = [
        profile_path.join("localstorage"),
        profile_path.join("logs"),
        profile_path.join("mediakeys"),
        profile_path.join("storage"),
        profile_path.join("nested").join("deep_dir"),
    ];

    for dir in &subdirs {
        fs::create_dir_all(dir).expect("Failed to create test directory");
        #[cfg(unix)]
        {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o755))
                .expect("Failed to set 0755 permissions");
        }
    }

    // Create mock sensitive profile files with permissive 0644 / 0666 permissions
    let test_files = [
        profile_path.join("syncify.db"),
        profile_path.join(".crypto_key"),
        profile_path.join("cookies"),
        profile_path.join("syncify.db-wal"),
        profile_path.join("syncify.db-shm"),
        profile_path.join("sync-spotify-0.writer.lock"),
        profile_path.join("logs").join("syncify.log"),
        profile_path.join("localstorage").join("http_localhost_5173.localstorage"),
    ];

    for file_path in &test_files {
        fs::write(file_path, b"mock sensitive content")
            .expect("Failed to write mock file");
        #[cfg(unix)]
        {
            fs::set_permissions(file_path, fs::Permissions::from_mode(0o644))
                .expect("Failed to set 0644 permissions");
        }
    }

    // Create a residual Spotify OAuth localstorage file to verify audit and purge
    let spotify_ls = profile_path
        .join("localstorage")
        .join("https_accounts.spotify.com_0.localstorage");
    fs::write(&spotify_ls, b"mock_spotify_webview_session_artifacts")
        .expect("Failed to create spotify localstorage file");
    assert!(spotify_ls.exists(), "Pre-condition: spotify localstorage file exists");

    // Execute hardening routine
    let report = ensure_secure_profile_permissions(profile_path)
        .expect("ensure_secure_profile_permissions failed");

    // Verify Spotify localstorage was purged
    assert!(
        !spotify_ls.exists(),
        "Residual Spotify WebView localstorage file must be purged by hardening routine"
    );
    assert!(
        report.purged_localstorage_files >= 1,
        "Report must reflect at least 1 purged localstorage file"
    );

    #[cfg(unix)]
    {
        // 1. Verify root profile directory has 0700
        let root_mode = fs::metadata(profile_path)
            .expect("Failed to read profile root metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            root_mode, 0o700,
            "Root profile directory must have exactly 0700 permissions"
        );

        // 2. Verify all subdirectories have 0700
        for dir in &subdirs {
            let dir_mode = fs::metadata(dir)
                .unwrap_or_else(|e| panic!("Failed to read metadata for {:?}: {}", dir, e))
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                dir_mode, 0o700,
                "Directory {:?} must have strictly 0700 permissions (got 0{:o})",
                dir, dir_mode
            );
        }

        // 3. Verify all files have 0600
        for file_path in &test_files {
            let file_mode = fs::metadata(file_path)
                .unwrap_or_else(|e| panic!("Failed to read metadata for {:?}: {}", file_path, e))
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                file_mode, 0o600,
                "File {:?} must have strictly 0600 permissions (got 0{:o})",
                file_path, file_mode
            );
        }
    }
}

#[test]
fn test_audit_and_purge_webview_localstorage_standalone() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let ls_dir = temp_dir.path().join("localstorage");
    fs::create_dir_all(&ls_dir).expect("Failed to create localstorage dir");

    let spotify_file = ls_dir.join("https_accounts.spotify.com_0.localstorage");
    let spotify_wal = ls_dir.join("https_accounts.spotify.com_0.localstorage-wal");
    let local_file = ls_dir.join("http_localhost_5173.localstorage");

    fs::write(&spotify_file, b"token=sp_dc_mock_value").expect("Failed to write mock spotify db");
    fs::write(&spotify_wal, b"wal_data").expect("Failed to write mock spotify wal");
    fs::write(&local_file, b"ui_state").expect("Failed to write mock local db");

    let purged = audit_and_purge_webview_localstorage(temp_dir.path())
        .expect("audit_and_purge_webview_localstorage failed");

    assert_eq!(purged, 2, "Expected 2 Spotify-related files to be purged");
    assert!(!spotify_file.exists(), "Spotify localstorage must be deleted");
    assert!(!spotify_wal.exists(), "Spotify localstorage-wal must be deleted");
    assert!(local_file.exists(), "Localhost app UI localstorage must be preserved");
}

#[test]
fn test_ensure_secure_profile_permissions_creates_missing_directory() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let non_existent_profile = temp_dir.path().join("new_profile_dir");

    assert!(!non_existent_profile.exists());

    let report = ensure_secure_profile_permissions(&non_existent_profile)
        .expect("ensure_secure_profile_permissions should create non-existent dir");

    assert!(non_existent_profile.exists());

    #[cfg(unix)]
    {
        let mode = fs::metadata(&non_existent_profile)
            .expect("Failed to read metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o700,
            "Created profile directory must have strictly 0700 permissions"
        );
        assert_eq!(report.directories_hardened, 1);
    }
}

#[test]
fn test_set_secure_process_umask_enforces_077() {
    // Calling set_secure_process_umask should be safe and idempotent
    set_secure_process_umask();

    #[cfg(unix)]
    {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let test_file = temp_dir.path().join("umask_test_file.txt");
        let test_dir = temp_dir.path().join("umask_test_dir");

        File::create(&test_file).expect("Failed to create test file");
        fs::create_dir(&test_dir).expect("Failed to create test dir");

        let file_mode = fs::metadata(&test_file)
            .expect("Metadata failed")
            .permissions()
            .mode()
            & 0o777;
        let dir_mode = fs::metadata(&test_dir)
            .expect("Metadata failed")
            .permissions()
            .mode()
            & 0o777;

        // With umask 0o077, group and others have 0 permissions
        assert_eq!(
            file_mode & 0o077,
            0,
            "Process umask 0o077 must prevent group/other access on newly created files (mode: 0{:o})",
            file_mode
        );
        assert_eq!(
            dir_mode & 0o077,
            0,
            "Process umask 0o077 must prevent group/other access on newly created dirs (mode: 0{:o})",
            dir_mode
        );
    }
}



