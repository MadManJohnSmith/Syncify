//! Security Test Suite for [TASK-90] / [SEC-006]
//! Restricción de Rutas y Confinamiento de Archivos en import_lyrics_file, write_m3u_to_disk y export_library
//!
//! Validates:
//! 1. `import_lyrics_file`:
//!    - Rejection of relative paths and path traversal (`..`).
//!    - Rejection of disallowed extensions (strictly `.lrc` or `.txt`).
//!    - Rejection of files exceeding 1 MB (`MAX_LYRICS_FILE_SIZE_BYTES`).
//!    - Rejection of unauthorized locations (/etc, ~/.ssh, etc.).
//!    - Success of legitimate `.lrc` and `.txt` imports from allowed directories.
//! 2. `write_m3u_to_disk`:
//!    - Rejection of relative paths and path traversal (`..`).
//!    - Rejection of extensions other than `.m3u` or `.m3u8`.
//!    - Rejection of unauthorized locations (/etc, ~/.ssh, etc.).
//!    - Success of legitimate `.m3u` and `.m3u8` playlist persistence in allowed directories.
//! 3. `export_library`:
//!    - Rejection of relative paths and path traversal (`..`).
//!    - Enforcement of `.json` extension and rejection of non-JSON extensions.
//!    - Rejection of unauthorized destinations (/etc, ~/.ssh, etc.).
//!    - Success of legitimate export with explicit path and default (None) path.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syncify_tauri_lib::commands::{
    export_library, get_allowed_backup_export_directories, get_allowed_lyrics_read_directories,
    get_allowed_m3u_directories, import_lyrics_file, validate_safe_backup_export_path,
    write_m3u_to_disk, MAX_LYRICS_FILE_SIZE_BYTES,
};
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use syncify_tauri_lib::EnrichmentWorkerState;
use tauri::Manager;

/// Resolves a writable sandbox-compliant directory inside Documents for test artifacts.
/// Each test receives its own isolated directory to avoid parallel test race conditions.
fn resolve_writable_test_dir(test_name: &str) -> PathBuf {
    let doc_dir = dirs::document_dir().expect("Documents directory must be resolvable");
    let base = if doc_dir.join("Syncify/target").exists() {
        doc_dir.join("Syncify/target/sec006_e2e_tests")
    } else {
        doc_dir.join("syncify_sec006_e2e_tests")
    };
    let test_dir = base.join(test_name);
    let _ = fs::create_dir_all(&test_dir);
    test_dir
}

/// Helper struct that removes test directories upon drop.
struct TestDirGuard(PathBuf);
impl Drop for TestDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Sets up an in-memory database with migrations through current schema and returns initialized AppState.
async fn setup_test_context() -> (tauri::App<tauri::test::MockRuntime>, SqlitePool, i64) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // Seed services and default account
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Test Account', 1)")
        .execute(&pool).await.unwrap();

    // Seed a sample artist, album, track
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('The Dark Side of the Moon') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Time', ?, 425000) RETURNING id"
    )
    .bind(album_id).fetch_one(&pool).await.unwrap();

    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);

    (app, pool, track_id)
}

// =========================================================================
// 1. LYRICS IMPORT CONFINEMENT TESTS (import_lyrics_file)
// =========================================================================

#[tokio::test]
async fn test_lyrics_import_relative_paths_rejected() {
    let (app, _pool, track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();

    let relative_cases = [
        "lyrics.lrc",
        "./lyrics.lrc",
        "../lyrics.lrc",
        "sub/lyrics.txt",
        "",
        "   ",
    ];

    for path in relative_cases {
        let res = import_lyrics_file(app_state.clone(), track_id, path.to_string()).await;
        assert!(
            res.is_err(),
            "Relative or empty path '{}' must be rejected",
            path
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Error for '{}' must indicate sandbox violation: {}",
            path,
            err
        );
    }
}

#[tokio::test]
async fn test_lyrics_import_path_traversal_rejected() {
    let (app, _pool, track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();
    let test_dir = resolve_writable_test_dir("lyrics_traversal");
    let _guard = TestDirGuard(test_dir.clone());

    let traversal_cases = [
        test_dir.join("../.bashrc").to_string_lossy().to_string(),
        test_dir.join("../../etc/passwd").to_string_lossy().to_string(),
        test_dir.join("sub/../../.ssh/id_rsa.txt").to_string_lossy().to_string(),
    ];

    for path in traversal_cases {
        let res = import_lyrics_file(app_state.clone(), track_id, path.clone()).await;
        assert!(
            res.is_err(),
            "Path traversal '{}' must be rejected",
            path
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Error must indicate sandbox violation: {}",
            err
        );
    }
}

#[tokio::test]
async fn test_lyrics_import_invalid_extensions_rejected() {
    let (app, _pool, track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();
    let test_dir = resolve_writable_test_dir("lyrics_invalid_ext");
    let _guard = TestDirGuard(test_dir.clone());

    let bad_extensions = [
        "payload.sh",
        "payload.exe",
        "payload.json",
        "payload.m3u",
        "payload.csv",
        "payload.py",
        "payload.bin",
        "payload",
        ".hidden_lyrics.lrc",
    ];

    for file_name in bad_extensions {
        let file_path = test_dir.join(file_name);
        let _ = fs::write(&file_path, "[00:01.00] Bad extension test");

        let res = import_lyrics_file(
            app_state.clone(),
            track_id,
            file_path.to_string_lossy().to_string(),
        )
        .await;

        assert!(
            res.is_err(),
            "File '{}' with invalid extension must be rejected",
            file_name
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Expected sandbox violation for '{}', got: {}",
            file_name,
            err
        );
    }
}

#[tokio::test]
async fn test_lyrics_import_oversized_file_rejected() {
    let (app, _pool, track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();
    let test_dir = resolve_writable_test_dir("lyrics_oversized");
    let _guard = TestDirGuard(test_dir.clone());

    let oversized_path = test_dir.join("oversized.lrc");
    // Create file exceeding 1 MB limit (1 MB + 1024 bytes)
    let payload = vec![b'A'; (MAX_LYRICS_FILE_SIZE_BYTES + 1024) as usize];
    fs::write(&oversized_path, payload).expect("Failed to write oversized test file");

    let res = import_lyrics_file(
        app_state,
        track_id,
        oversized_path.to_string_lossy().to_string(),
    )
    .await;

    assert!(
        res.is_err(),
        "Lyrics file exceeding 1 MB must be strictly rejected"
    );
    let err = res.unwrap_err();
    assert!(
        err.contains("1 MB") && (err.contains("sandbox violation") || err.contains("Acceso denegado")),
        "Error must mention 1 MB size limit and sandbox violation, got: {}",
        err
    );
}

#[tokio::test]
async fn test_lyrics_import_unauthorized_system_paths_rejected() {
    let (app, _pool, track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();

    let sensitive_paths = [
        "/etc/passwd",
        "/etc/shadow.txt",
        "/etc/hosts.lrc",
        "/var/log/syslog.txt",
        "/dev/random.txt",
        "/proc/cpuinfo.txt",
    ];

    for path in sensitive_paths {
        let res = import_lyrics_file(app_state.clone(), track_id, path.to_string()).await;
        assert!(
            res.is_err(),
            "Access to system sensitive path '{}' must be rejected",
            path
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation")
                || err.contains("Acceso denegado")
                || err.contains("no existe"),
            "Expected confinement rejection for '{}', got: {}",
            path,
            err
        );
    }
}

#[tokio::test]
async fn test_lyrics_import_legitimate_files_succeed() {
    let (app, _pool, track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();
    let test_dir = resolve_writable_test_dir("lyrics_legitimate");
    let _guard = TestDirGuard(test_dir.clone());

    // 1. Valid .lrc file
    let valid_lrc_path = test_dir.join("song.lrc");
    let lrc_content = "[00:12.34] Ticking away the moments that make up a dull day\n[00:16.78] Fritter and waste the hours in an offhand way";
    fs::write(&valid_lrc_path, lrc_content).expect("Failed to write valid .lrc");

    let lrc_res = import_lyrics_file(
        app_state.clone(),
        track_id,
        valid_lrc_path.to_string_lossy().to_string(),
    )
    .await;

    assert!(lrc_res.is_ok(), "Legitimate .lrc import must succeed: {:?}", lrc_res.err());
    let lyrics = lrc_res.unwrap();
    assert_eq!(lyrics.format, "lrc");
    assert_eq!(lyrics.sync_level, Some("line".to_string()));
    assert!(lyrics.content.contains("Ticking away"));

    // 2. Valid .txt file
    let valid_txt_path = test_dir.join("song.txt");
    let txt_content = "Plain text lyrics\nWithout timestamps\nFor testing purposes";
    fs::write(&valid_txt_path, txt_content).expect("Failed to write valid .txt");

    let txt_res = import_lyrics_file(
        app_state,
        track_id,
        valid_txt_path.to_string_lossy().to_string(),
    )
    .await;

    assert!(txt_res.is_ok(), "Legitimate .txt import must succeed: {:?}", txt_res.err());
    let txt_lyrics = txt_res.unwrap();
    assert_eq!(txt_lyrics.format, "plain");
    assert_eq!(txt_lyrics.sync_level, Some("none".to_string()));
    assert!(txt_lyrics.content.contains("Plain text lyrics"));
}

// =========================================================================
// 2. PLAYLIST M3U CONFINEMENT TESTS (write_m3u_to_disk)
// =========================================================================

#[test]
fn test_m3u_write_relative_and_traversal_paths_rejected() {
    let test_dir = resolve_writable_test_dir("m3u_traversal");
    let _guard = TestDirGuard(test_dir.clone());

    let invalid_cases = vec![
        "playlist.m3u".to_string(),
        "./playlist.m3u".to_string(),
        "../playlist.m3u".to_string(),
        "sub/playlist.m3u8".to_string(),
        "".to_string(),
        "   ".to_string(),
        test_dir.join("../escape.m3u").to_string_lossy().to_string(),
        test_dir.join("../../etc/cron.m3u").to_string_lossy().to_string(),
    ];

    for path in &invalid_cases {
        let res = write_m3u_to_disk(path, "#EXTM3U\n#EXTINF:100,Test\n/song.flac");
        assert!(
            res.is_err(),
            "Path '{}' must be rejected by write_m3u_to_disk",
            path
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Expected sandbox violation for '{}', got: {}",
            path,
            err
        );
    }
}

#[test]
fn test_m3u_write_invalid_extensions_rejected() {
    let test_dir = resolve_writable_test_dir("m3u_invalid_ext");
    let _guard = TestDirGuard(test_dir.clone());

    let bad_cases = [
        "playlist.txt",
        "playlist.json",
        "playlist.sh",
        "playlist.csv",
        "playlist.mp3",
        "playlist",
        ".hidden.m3u",
    ];

    for file_name in bad_cases {
        let path = test_dir.join(file_name).to_string_lossy().to_string();
        let res = write_m3u_to_disk(&path, "#EXTM3U\n");
        assert!(
            res.is_err(),
            "write_m3u_to_disk must reject non-m3u extension for '{}'",
            file_name
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Expected sandbox violation for '{}', got: {}",
            file_name,
            err
        );
    }
}

#[test]
fn test_m3u_write_unauthorized_destinations_rejected() {
    let sensitive_cases = [
        "/etc/malicious.m3u",
        "/etc/cron.d/test.m3u8",
        "/var/log/test.m3u",
        "/tmp/outside_user_dirs.m3u",
    ];

    for path in sensitive_cases {
        let res = write_m3u_to_disk(path, "#EXTM3U\n");
        assert!(
            res.is_err(),
            "Destination '{}' outside authorized user directories must be rejected",
            path
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Expected sandbox violation for '{}', got: {}",
            path,
            err
        );
    }
}

#[test]
fn test_m3u_write_legitimate_files_succeed() {
    let test_dir = resolve_writable_test_dir("m3u_legitimate");
    let _guard = TestDirGuard(test_dir.clone());

    // Test .m3u
    let m3u_path = test_dir.join("favorites.m3u");
    let m3u_content = "#EXTM3U\n#EXTINF:250,Artist - Title\nMusic/Artist/Album/01 Track.flac\n";
    let res_m3u = write_m3u_to_disk(m3u_path.to_str().unwrap(), m3u_content);
    assert!(res_m3u.is_ok(), "Writing legitimate .m3u must succeed: {:?}", res_m3u.err());
    assert_eq!(fs::read_to_string(&m3u_path).unwrap(), m3u_content);

    // Test .m3u8
    let m3u8_path = test_dir.join("favorites_utf8.m3u8");
    let m3u8_content = "#EXTM3U\n#EXTINF:180,Café Tacvba - Eres\nMusic/Cafe Tacvba/Eres.flac\n";
    let res_m3u8 = write_m3u_to_disk(m3u8_path.to_str().unwrap(), m3u8_content);
    assert!(res_m3u8.is_ok(), "Writing legitimate .m3u8 must succeed: {:?}", res_m3u8.err());
    assert_eq!(fs::read_to_string(&m3u8_path).unwrap(), m3u8_content);
}

// =========================================================================
// 3. BACKUP EXPORT CONFINEMENT TESTS (export_library)
// =========================================================================

#[tokio::test]
async fn test_backup_export_relative_and_traversal_paths_rejected() {
    let (app, _pool, _track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();
    let test_dir = resolve_writable_test_dir("backup_traversal");
    let _guard = TestDirGuard(test_dir.clone());

    let invalid_cases = vec![
        "backup.json".to_string(),
        "./backup.json".to_string(),
        "../backup.json".to_string(),
        "sub/backup.json".to_string(),
        "".to_string(),
        "   ".to_string(),
        test_dir.join("../escape.json").to_string_lossy().to_string(),
        test_dir.join("../../etc/backup.json").to_string_lossy().to_string(),
    ];

    for path in invalid_cases {
        let res = export_library(app_state.clone(), Some(path.clone())).await;
        assert!(
            res.is_err(),
            "export_library must reject invalid/traversal path '{}'",
            path
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Expected sandbox violation for '{}', got: {}",
            path,
            err
        );
    }
}

#[tokio::test]
async fn test_backup_export_invalid_extensions_rejected() {
    let (app, _pool, _track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();
    let test_dir = resolve_writable_test_dir("backup_invalid_ext");
    let _guard = TestDirGuard(test_dir.clone());

    let bad_cases = [
        "backup.xml",
        "backup.txt",
        "backup.csv",
        "backup.sh",
        "backup.sql",
        "backup",
        ".hidden_backup.json",
    ];

    for file_name in bad_cases {
        let path = test_dir.join(file_name).to_string_lossy().to_string();
        let res = export_library(app_state.clone(), Some(path.clone())).await;
        assert!(
            res.is_err(),
            "export_library must reject non-json extension for '{}'",
            file_name
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Expected sandbox violation for '{}', got: {}",
            file_name,
            err
        );
    }
}

#[tokio::test]
async fn test_backup_export_unauthorized_destinations_rejected() {
    let (app, _pool, _track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();

    let sensitive_cases = [
        "/etc/backup.json",
        "/var/backups/backup.json",
        "/tmp/unconfined_backup.json",
    ];

    for path in sensitive_cases {
        let res = export_library(app_state.clone(), Some(path.to_string())).await;
        assert!(
            res.is_err(),
            "Destination '{}' outside authorized user directories must be rejected",
            path
        );
        let err = res.unwrap_err();
        assert!(
            err.contains("sandbox violation") || err.contains("Acceso denegado"),
            "Expected sandbox violation for '{}', got: {}",
            path,
            err
        );
    }
}

#[tokio::test]
async fn test_backup_export_legitimate_destinations_succeed() {
    let (app, _pool, _track_id) = setup_test_context().await;
    let app_state = app.state::<AppState>();
    let test_dir = resolve_writable_test_dir("backup_legitimate");
    let _guard = TestDirGuard(test_dir.clone());

    // 1. Explicit valid destination path in allowed directory
    let custom_backup_path = test_dir.join("library_backup_test.json");
    let custom_res = export_library(
        app_state.clone(),
        Some(custom_backup_path.to_string_lossy().to_string()),
    )
    .await;

    assert!(custom_res.is_ok(), "export_library with valid custom path must succeed: {:?}", custom_res.err());
    let custom_output = custom_res.unwrap();
    assert!(Path::new(&custom_output.file_path).exists());
    assert!(custom_output.file_size_bytes > 0);
    assert!(!custom_output.checksum.is_empty());

    let content = fs::read_to_string(&custom_output.file_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("Must be valid JSON");
    assert_eq!(parsed["version"], "1.0.0");
    assert_eq!(parsed["tracks"].as_array().unwrap().len(), 1);

    // 2. Default destination path (output_path == None)
    // In test environment under workspace-write sandbox, dirs::download_dir() is mounted read-only.
    // We execute export_library(app_state, None) and verify path validation approved the default path.
    let default_output = export_library(app_state, None).await;
    match default_output {
        Ok(res) => {
            let default_file = Path::new(&res.file_path);
            assert!(default_file.exists());
            assert!(res.file_path.ends_with(".json"));
            let _ = fs::remove_file(default_file);
        }
        Err(e) if e.contains("Read-only file system") || e.contains("os error 30") => {
            // Path validation passed, but sandbox prevented disk write to Downloads
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let filename = format!("Syncify_Backup_{}.json", timestamp);
            let default_dir = dirs::download_dir().unwrap();
            let default_path = default_dir.join(filename);
            let validated = validate_safe_backup_export_path(&default_path);
            assert!(validated.is_ok(), "Default path must pass safe validation: {:?}", validated.err());
        }
        Err(e) => panic!("Unexpected error on default export: {}", e),
    }
}

// =========================================================================
// 4. VERIFY ALLOWED DIRECTORIES DISCOVERY CONTRACT
// =========================================================================

#[test]
fn test_allowed_directory_resolvers_are_non_empty_and_absolute() {
    let lyrics_dirs = get_allowed_lyrics_read_directories();
    assert!(!lyrics_dirs.is_empty(), "Lyrics allowed directories must not be empty");
    for dir in &lyrics_dirs {
        assert!(dir.is_absolute(), "Directory must be absolute: {:?}", dir);
    }

    let m3u_dirs = get_allowed_m3u_directories();
    assert!(!m3u_dirs.is_empty(), "M3U allowed directories must not be empty");
    for dir in &m3u_dirs {
        assert!(dir.is_absolute(), "Directory must be absolute: {:?}", dir);
    }

    let backup_dirs = get_allowed_backup_export_directories();
    assert!(!backup_dirs.is_empty(), "Backup allowed directories must not be empty");
    for dir in &backup_dirs {
        assert!(dir.is_absolute(), "Directory must be absolute: {:?}", dir);
    }
}
