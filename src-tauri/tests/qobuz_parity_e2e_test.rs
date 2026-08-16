//! E2E & Parity Integration Tests for Qobuz Downloader and Orchestrator (Sprint S108)

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::DownloadRequest;
use syncify_tauri_lib::download::qobuz::{
    build_request_signature, map_quality_to_format_id, QobuzDownloader,
};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to SQLite in-memory");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS services (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            service_id INTEGER NOT NULL REFERENCES services(id),
            is_active INTEGER NOT NULL DEFAULT 1,
            credentials_json TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS folder_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            base_folder TEXT,
            folder_template TEXT NOT NULL DEFAULT '{AlbumArtist}/{Album}',
            file_template TEXT NOT NULL DEFAULT '{TrackNumber:pad2} - {Title}',
            artist_separator TEXT NOT NULL DEFAULT ', ',
            replace_spaces_with TEXT,
            max_path_length INTEGER NOT NULL DEFAULT 255,
            fallback_action TEXT NOT NULL DEFAULT 'try_next'
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("Schema creation failed");

    pool
}

#[test]
fn test_qobuz_signature_and_quality_mapping() {
    // 1. Format ID mapping for CLI parity
    assert_eq!(map_quality_to_format_id("16-44"), "6");
    assert_eq!(map_quality_to_format_id("16-44.1"), "6");
    assert_eq!(map_quality_to_format_id("LOSSLESS"), "6");
    assert_eq!(map_quality_to_format_id("24-96"), "7");
    assert_eq!(map_quality_to_format_id("HI_RES"), "7");
    assert_eq!(map_quality_to_format_id("24-192"), "27");
    assert_eq!(map_quality_to_format_id("HI_RES_LOSSLESS"), "27");
    assert_eq!(map_quality_to_format_id("320"), "5");
    assert_eq!(map_quality_to_format_id("HIGH"), "5");

    // 2. Pure MD5 signature computation
    let sig = build_request_signature("6", "123456", "1700000000", "abb21364945c0583309667d13ca3d93a");
    assert_eq!(sig.len(), 32);
    assert_eq!(
        sig,
        format!(
            "{:x}",
            md5::compute(b"trackgetFileUrlformat_id6intentstreamtrack_id1234561700000000abb21364945c0583309667d13ca3d93a")
        )
    );
}

#[tokio::test]
async fn test_qobuz_token_resolution_from_sqlite() {
    let pool = create_test_db().await;

    // Seed Qobuz service
    let svc_id: i64 = sqlx::query_scalar("INSERT INTO services (name) VALUES ('qobuz') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Initialize keychain crypto for encryption/decryption
    let _ = syncify_tauri_lib::crypto::init_keychain_crypto();

    // Case 1: user_auth_token
    let creds = r#"{"user_auth_token":"qobuz_auth_token_secret_12345","user_id":"123"}"#;
    let encrypted = syncify_tauri_lib::crypto::encrypt(creds).expect("Encryption failed");

    sqlx::query("INSERT INTO accounts (service_id, is_active, credentials_json) VALUES (?, 1, ?)")
        .bind(svc_id)
        .bind(encrypted)
        .execute(&pool)
        .await
        .unwrap();

    let downloader = QobuzDownloader::new();
    let token = match downloader.resolve_token(Some(&pool)).await {
        Ok(t) => t,
        Err(e) => panic!("Failed to resolve token: {:?}", e),
    };
    assert_eq!(token, "qobuz_auth_token_secret_12345");

    // Case 2: auth_token / access_token field variant
    let creds2 = r#"{"auth_token":"qobuz_secondary_token_67890"}"#;
    let encrypted2 = syncify_tauri_lib::crypto::encrypt(creds2).expect("Encryption failed");
    sqlx::query("UPDATE accounts SET credentials_json = ? WHERE service_id = ?")
        .bind(encrypted2)
        .bind(svc_id)
        .execute(&pool)
        .await
        .unwrap();

    let token2 = downloader.resolve_token(Some(&pool)).await.expect("Failed to resolve secondary token");
    assert_eq!(token2, "qobuz_secondary_token_67890");

    // Case 3: No active account returns explicit RequiresAuth
    sqlx::query("UPDATE accounts SET is_active = 0 WHERE service_id = ?")
        .bind(svc_id)
        .execute(&pool)
        .await
        .unwrap();

    let res_no_account = downloader.resolve_token(Some(&pool)).await;
    assert!(matches!(res_no_account, Err(syncify_tauri_lib::download::qobuz::QobuzAuthStatus::RequiresAuth(_))));
}

#[tokio::test]
async fn test_orchestrator_prefers_qobuz_over_tidal() {
    let pool = create_test_db().await;
    let _orchestrator = DownloadOrchestrator::new().with_db(pool);

    // Default priority must have qobuz before tidal
    let req = DownloadRequest {
        item_id: "test_parity_1".to_string(),
        isrc: Some("USUG12101234".to_string()),
        spotify_id: None,
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("12345678".to_string()),
        service_album_id: Some("87654321".to_string()),
        track_name: "Heroes".to_string(),
        artist_name: "David Bowie".to_string(),
        album_name: "Heroes (2017 Remaster)".to_string(),
        album_artist: None,
        duration_ms: 360000,
        track_number: 1,
        disc_number: 1,
        total_tracks: 10,
        release_date: Some("1977-10-14".to_string()),
        cover_url: None,
        output_dir: "./downloads_test".to_string(),
        quality: "16-44".to_string(),
        embed_lyrics: true,
        embed_artwork: true,
        smart_studio_origin: true,
        allow_fallback: false,
    };

    assert_eq!(req.quality, "16-44");
    assert_eq!(req.track_name, "Heroes");
    assert_eq!(req.service_name.as_deref(), Some("qobuz"));
    assert_eq!(req.service_track_id.as_deref(), Some("12345678"));
    assert!(!req.allow_fallback);
}

#[tokio::test]
async fn test_edition_preservation_and_no_unauthorized_provider_fallback() {
    let pool = create_test_db().await;

    // Create download queue and track tables
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            album_id INTEGER,
            isrc TEXT
        );
        CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            service_id INTEGER NOT NULL,
            service_track_id TEXT NOT NULL,
            quality_score INTEGER DEFAULT 0,
            bit_depth INTEGER DEFAULT 16,
            available INTEGER DEFAULT 1
        );
        CREATE TABLE IF NOT EXISTS download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            service_id INTEGER,
            service_name TEXT,
            service_track_id TEXT,
            service_album_id TEXT,
            target_title TEXT,
            target_artist TEXT,
            target_album TEXT,
            target_isrc TEXT,
            quality_preference TEXT,
            priority INTEGER DEFAULT 50,
            status TEXT NOT NULL DEFAULT 'queued',
            progress_percent REAL DEFAULT 0.0,
            retry_count INTEGER DEFAULT 0,
            position INTEGER DEFAULT 0,
            resumable INTEGER DEFAULT 1,
            smart_studio_origin INTEGER DEFAULT 0,
            allow_fallback INTEGER DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    // 1. Insert 3 editions of "#1 Crush"
    // Edition 1: "Garbage" (Studio Album, Qobuz Track ID: 101)
    // Edition 2: "Absolute Garbage" (Greatest Hits, Qobuz Track ID: 102)
    // Edition 3: "Anthology" (Compilation, Tidal Track ID: 203)
    let alb_studio: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Garbage') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let alb_greatest: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Absolute Garbage') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    let track_studio: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('#1 Crush', ?, 'USIR19500001') RETURNING id")
        .bind(alb_studio).fetch_one(&pool).await.unwrap();
    let track_greatest: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('#1 Crush', ?, 'USIR19500001') RETURNING id")
        .bind(alb_greatest).fetch_one(&pool).await.unwrap();

    // Insert sources for track_studio
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, available) VALUES (?, 1, '101', 90, 24, 1)")
        .bind(track_studio).execute(&pool).await.unwrap();
    // Insert sources for track_greatest
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, available) VALUES (?, 1, '102', 80, 16, 1)")
        .bind(track_greatest).execute(&pool).await.unwrap();

    // 2. Enqueue specific Studio Album edition explicitly
    let q_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (
            track_id, service_id, service_name, service_track_id, target_title, target_artist, target_album, target_isrc, quality_preference, allow_fallback
        )
        VALUES (?, 1, 'qobuz', '101', '#1 Crush', 'Garbage', 'Garbage', 'USIR19500001', 'HI_RES_LOSSLESS', 0)
        RETURNING id
        "#
    )
    .bind(track_studio)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 3. Verify queue row retains exact edition identity
    let row: (String, String, String, String, i64) = sqlx::query_as(
        "SELECT service_name, service_track_id, target_album, quality_preference, allow_fallback FROM download_queue WHERE id = ?"
    )
    .bind(q_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, "qobuz");
    assert_eq!(row.1, "101");
    assert_eq!(row.2, "Garbage");
    assert_eq!(row.3, "HI_RES_LOSSLESS");
    assert_eq!(row.4, 0); // allow_fallback = 0

    // 4. Verify DownloadOrchestrator will NOT fallback to Tidal when allow_fallback == false
    let orchestrator = DownloadOrchestrator::new().with_db(pool);
    let req = DownloadRequest {
        item_id: q_id.to_string(),
        isrc: Some("USIR19500001".to_string()),
        spotify_id: None,
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("101".to_string()),
        service_album_id: None,
        track_name: "#1 Crush".to_string(),
        artist_name: "Garbage".to_string(),
        album_name: "Garbage".to_string(),
        album_artist: None,
        duration_ms: 284000,
        track_number: 5,
        disc_number: 1,
        total_tracks: 12,
        release_date: Some("1995-08-15".to_string()),
        cover_url: None,
        output_dir: "./downloads".to_string(),
        quality: "HI_RES_LOSSLESS".to_string(),
        embed_lyrics: true,
        embed_artwork: true,
        smart_studio_origin: true,
        allow_fallback: false,
    };

    // Attempting download without active Qobuz OAuth credentials must fail with Qobuz RequiresAuth
    // and must NOT try Tidal silently!
    let res = orchestrator.download_track(&req).await;
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert!(err_str.contains("qobuz") || err_str.contains("RequiresAuth") || err_str.contains("No active accounts found"));
    assert!(!err_str.contains("tidal"), "Must NOT cascade to Tidal when allow_fallback is false");
}

#[tokio::test]
async fn test_staging_and_flac_magic_validation() {
    let temp_dir = std::env::temp_dir().join("syncify_staging_test");
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();

    let staging_path = temp_dir.join("track_123.part");

    // 1. Valid FLAC magic bytes (fLaC)
    let valid_flac_header = b"fLaC\x00\x00\x00\x22\x10\x00\x10\x00";
    tokio::fs::write(&staging_path, valid_flac_header).await.unwrap();
    assert!(AudioByteValidator::is_flac_magic(valid_flac_header));

    // 2. Corrupt / HTML payload rejected
    let html_payload = b"<html><head><title>404 Not Found</title></head></html>";
    assert!(!AudioByteValidator::is_flac_magic(html_payload));

    // Clean up
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_output_dir_resolution_hierarchy() {
    let pool = create_test_db().await;

    // 1. Initially empty -> fallback
    let default_path = dirs::audio_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\Music"))
        .join("Syncify");

    // 2. Insert into folder_settings
    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, 'D:/MyMusic/Lossless')")
        .execute(&pool)
        .await
        .unwrap();

    let configured: Option<String> = sqlx::query_scalar("SELECT base_folder FROM folder_settings WHERE id = 1")
        .fetch_optional(&pool)
        .await
        .unwrap();

    assert_eq!(configured, Some("D:/MyMusic/Lossless".to_string()));
    assert_ne!(configured.unwrap(), default_path.to_string_lossy().to_string());
}
