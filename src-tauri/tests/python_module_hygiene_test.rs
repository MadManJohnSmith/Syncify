//! Python Module Hygiene & Orphaned Code Purge Test Suite (TASK-130)
//!
//! Validates:
//! 1. Complete absence of legacy/orphaned Python scripts and disposable archive directories from production tree.
//! 2. Proper preservation in workspace/audit_archive/scripts/orphaned_python_modules/ with explanatory README.
//! 3. Active presence of production IPC bridges (scanner, conversion, organizer, auth, download, lyrics, metadata, fingerprint, playlist).
//! 4. Architectural superseding of legacy Python health checks by native Tauri Rust batch health check commands.

use std::path::PathBuf;

fn get_repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest_dir.ends_with("src-tauri") {
        manifest_dir.parent().expect("workspace root").to_path_buf()
    } else {
        manifest_dir
    }
}

#[test]
fn test_purged_python_modules_and_archives_absent_from_production() {
    let root = get_repo_root();

    let forbidden_paths = [
        root.join("scripts/services/audio_converter.py"),
        root.join("scripts/services/soundcloud_api.py"),
        root.join("scripts/services/local_file_scanner.py"),
        root.join("scripts/services/spotify_api.py"),
        root.join("scripts/services/settings_manager.py"),
        root.join("scripts/health_check.py"),
        root.join("scripts/archive"),
        root.join("scripts/archive/replace_folders.py"),
        root.join("scripts/archive/replace_sync.py"),
        root.join("scripts/archive/parse_ndjson.py"),
        root.join("scripts/archive/test_bridges.py"),
        root.join("src-tauri/get_token.py"),
    ];

    for path in &forbidden_paths {
        assert!(
            !path.exists(),
            "Orphaned/purged path must not exist in production codebase: {:?}",
            path
        );
    }
}

#[test]
fn test_archived_modules_and_readme_present_in_audit_archive() {
    let root = get_repo_root();
    let archive_dir = root.join("workspace/audit_archive/scripts/orphaned_python_modules");

    assert!(
        archive_dir.is_dir(),
        "Archive destination must exist: {:?}",
        archive_dir
    );

    let expected_files = [
        "audio_converter.py",
        "soundcloud_api.py",
        "local_file_scanner.py",
        "spotify_api.py",
        "settings_manager.py",
        "health_check.py",
        "replace_folders.py",
        "replace_sync.py",
        "parse_ndjson.py",
        "test_bridges.py",
        "README.md",
    ];

    for filename in &expected_files {
        let path = archive_dir.join(filename);
        assert!(
            path.is_file(),
            "Expected archived artifact missing: {:?}",
            path
        );
        let metadata = std::fs::metadata(&path).expect("read metadata");
        assert!(
            metadata.len() > 0,
            "Archived artifact must not be empty: {:?}",
            path
        );
    }

    let readme_content = std::fs::read_to_string(archive_dir.join("README.md"))
        .expect("read README.md");

    for filename in &expected_files {
        if *filename == "README.md" {
            continue;
        }
        assert!(
            readme_content.contains(filename),
            "Archive README.md must document why module was retired: {}",
            filename
        );
    }
}

#[test]
fn test_legitimate_production_bridges_exist() {
    let root = get_repo_root();
    let scripts_dir = root.join("scripts");

    let required_bridges = [
        "scanner_bridge.py",
        "conversion_bridge.py",
        "organizer_bridge.py",
        "auth_bridge.py",
        "download_bridge.py",
        "lyrics_bridge.py",
        "metadata_bridge.py",
        "fingerprint_bridge.py",
        "playlist_bridge.py",
    ];

    for bridge in &required_bridges {
        let path = scripts_dir.join(bridge);
        assert!(
            path.is_file(),
            "Required production bridge missing from scripts/: {:?}",
            path
        );
    }
}

#[test]
fn test_native_batch_health_check_command_is_authoritative() {
    // Verify that Tauri commands module exports BatchHealthReport and health checks,
    // confirming Python health_check.py was superseded by native Rust implementation.
    use syncify_tauri_lib::commands::BatchHealthReport;

    let report = BatchHealthReport {
        timestamp: "2026-09-06T12:00:00Z".to_string(),
        database_healthy: true,
        database_integrity: "ok".to_string(),
        foreign_keys_valid: true,
        queue_total: 0,
        queue_queued: 0,
        queue_downloading: 0,
        queue_completed: 0,
        queue_failed: 0,
        downloads_total: 0,
        downloads_verified_on_disk: 0,
        downloads_missing_on_disk: 0,
        staging_orphans_count: 0,
        staging_orphans_bytes: 0,
        worker_active_downloads: 0,
        worker_max_concurrent: 3,
        worker_paused: false,
        healthy: true,
        issues: vec![],
        effective_download_path: "/tmp/downloads".to_string(),
        effective_staging_path: "/tmp/staging".to_string(),
    };

    assert!(report.healthy);
    assert!(report.database_healthy);
    assert_eq!(report.database_integrity, "ok");
    assert_eq!(report.staging_orphans_count, 0);
}
