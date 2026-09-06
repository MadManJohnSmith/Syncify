//! E2E Test Suite for Sprint S101: Backup & Restore de Biblioteca
//!
//! Validates manifest serialization, SHA-256 checksum hashing, cross-machine import,
//! deduplication idempotence, and transactional safety using production commands.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use syncify_tauri_lib::{
    commands::backup::{export_library, import_library},
    worker::DownloadWorkerState,
    AppState, EnrichmentWorkerState,
};
use tauri::Manager;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through current must apply cleanly");

    // Seed services & default account
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Test Account', 1)")
        .execute(&pool).await.unwrap();

    pool
}

fn create_test_app(pool: SqlitePool) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);
    app
}

fn get_test_backup_path(prefix: &str) -> String {
    let base = dirs::document_dir()
        .map(|d| d.join("Syncify").join("target"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let _ = std::fs::create_dir_all(&base);
    base.join(format!("{}_{}.json", prefix, uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string()
}

#[tokio::test]
async fn test_backup_export_schema_and_checksum_integrity() {
    let db = create_test_db().await;

    // Seed test data
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, favorite_at) VALUES ('David Bowie', '2026-08-15T12:00:00Z') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc, release_date, favorite_at) VALUES ('Heroes', '012345678901', '1977-10-14', '2026-08-15T12:05:00Z') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let _ = sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc, duration_ms, track_number, disc_number, favorite_at) VALUES ('Heroes', ?, 'GBAYE7700037', 368000, 3, 1, '2026-08-15T12:10:00Z') RETURNING id"
    )
    .bind(album_id).fetch_one(&db).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id).bind(artist_id).execute(&db).await.unwrap();

    let app = create_test_app(db);
    let app_state = app.state::<AppState>();

    // Invoke production export_library command
    let dest = get_test_backup_path("test_backup_schema");
    let res = export_library(app_state, Some(dest.clone())).await.expect("export_library must succeed");
    assert_eq!(res.tracks_count, 1);
    assert_eq!(res.albums_count, 1);
    assert_eq!(res.artists_count, 1);
    assert_eq!(res.checksum.len(), 64, "SHA-256 hex string must be 64 characters");

    let export_path = std::path::Path::new(&res.file_path);
    assert!(export_path.exists(), "Exported manifest file must exist on disk");

    let raw = std::fs::read_to_string(export_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(manifest["version"], "1.0.0");
    assert_eq!(manifest["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["tracks"][0]["title"], "Heroes");

    let _ = std::fs::remove_file(export_path);
}

#[tokio::test]
async fn test_backup_restore_on_clean_db_recreates_library_state() {
    let source_db = create_test_db().await;

    let art_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name, favorite_at) VALUES ('Queen', '2026-08-15T10:00:00Z') RETURNING id"
    ).fetch_one(&source_db).await.unwrap();

    let alb_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, upc, release_date, favorite_at) VALUES ('A Night at the Opera', '0123456789', '1975-11-21', '2026-08-15T10:05:00Z') RETURNING id"
    ).fetch_one(&source_db).await.unwrap();
    let _ = sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_id).bind(art_id).execute(&source_db).await.unwrap();

    let trk_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc, duration_ms, track_number, disc_number, favorite_at) VALUES ('Bohemian Rhapsody', ?, 'GBUM71029604', 354000, 11, 1, '2026-08-15T10:10:00Z') RETURNING id"
    ).bind(alb_id).fetch_one(&source_db).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk_id).bind(art_id).execute(&source_db).await.unwrap();

    let source_app = create_test_app(source_db);
    let dest = get_test_backup_path("test_backup_restore");
    let export_res = export_library(source_app.state::<AppState>(), Some(dest)).await.unwrap();

    // Create fresh target DB and app
    let target_db = create_test_db().await;
    let track_count_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks").fetch_one(&target_db).await.unwrap();
    assert_eq!(track_count_before.0, 0);

    let target_app = create_test_app(target_db.clone());
    let import_res = import_library(target_app.state::<AppState>(), export_res.file_path.clone(), None)
        .await
        .expect("import_library must succeed");

    assert_eq!(import_res.tracks_imported, 1);
    assert_eq!(import_res.albums_imported, 1);
    assert_eq!(import_res.artists_imported, 1);
    assert_eq!(import_res.favorites_restored, 3); // artist, album, track

    // Verify recreated state in target DB
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE isrc = 'GBUM71029604'").fetch_one(&target_db).await.unwrap();
    assert_eq!(count.0, 1);

    let fav: (Option<String>,) = sqlx::query_as("SELECT favorite_at FROM tracks WHERE isrc = 'GBUM71029604'").fetch_one(&target_db).await.unwrap();
    assert_eq!(fav.0, Some("2026-08-15T10:10:00Z".to_string()));

    let _ = std::fs::remove_file(&export_res.file_path);
}

#[tokio::test]
async fn test_backup_restore_idempotence_and_deduplication() {
    let source_db = create_test_db().await;

    let art_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name, favorite_at) VALUES ('Led Zeppelin', '2026-08-15T10:00:00Z') RETURNING id"
    ).fetch_one(&source_db).await.unwrap();
    let alb_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, upc) VALUES ('Led Zeppelin IV', '075678263822') RETURNING id"
    ).fetch_one(&source_db).await.unwrap();
    let _ = sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_id).bind(art_id).execute(&source_db).await.unwrap();
    let trk_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc) VALUES ('Stairway to Heaven', ?, 'USAT20000001') RETURNING id"
    ).bind(alb_id).fetch_one(&source_db).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk_id).bind(art_id).execute(&source_db).await.unwrap();

    let source_app = create_test_app(source_db);
    let dest = get_test_backup_path("test_backup_idempotence");
    let export_res = export_library(source_app.state::<AppState>(), Some(dest)).await.unwrap();

    let target_db = create_test_db().await;
    let target_app = create_test_app(target_db.clone());

    // Import 1
    let res1 = import_library(target_app.state::<AppState>(), export_res.file_path.clone(), None).await.unwrap();
    assert_eq!(res1.tracks_imported, 1);

    // Import 2 (idempotent replay)
    let res2 = import_library(target_app.state::<AppState>(), export_res.file_path.clone(), None).await.unwrap();
    assert_eq!(res2.tracks_imported, 1);

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE isrc = 'USAT20000001'").fetch_one(&target_db).await.unwrap();
    assert_eq!(count.0, 1, "Idempotent import must not produce duplicate tracks");

    let _ = std::fs::remove_file(&export_res.file_path);
}

#[tokio::test]
async fn test_backup_restore_corrupted_checksum_fails_safely() {
    let source_db = create_test_db().await;
    let aid: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('The Rolling Stones') RETURNING id")
        .fetch_one(&source_db).await.unwrap();
    let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Original Song', 'GBTEST0001') RETURNING id")
        .fetch_one(&source_db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(tid).bind(aid).execute(&source_db).await.unwrap();

    let source_app = create_test_app(source_db);
    let dest = get_test_backup_path("test_backup_corrupt");
    let export_res = export_library(source_app.state::<AppState>(), Some(dest)).await.unwrap();

    // Tamper with exported file content without updating checksum
    let raw = std::fs::read_to_string(&export_res.file_path).unwrap();
    let tampered = raw.replace("Original Song", "Malicious Injected Song");
    std::fs::write(&export_res.file_path, tampered).unwrap();

    let target_db = create_test_db().await;
    let target_app = create_test_app(target_db.clone());

    // Production import_library must detect checksum tampering and reject
    let import_result = import_library(target_app.state::<AppState>(), export_res.file_path.clone(), None).await;
    assert!(import_result.is_err(), "Tampered backup file must be rejected by checksum validation");
    let err_msg = import_result.err().unwrap();
    assert!(err_msg.contains("checksum mismatch"), "Error must report checksum mismatch");

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks").fetch_one(&target_db).await.unwrap();
    assert_eq!(count.0, 0, "No tracks should be imported from a tampered manifest");

    let _ = std::fs::remove_file(&export_res.file_path);
}

#[tokio::test]
async fn test_backup_restore_playlist_with_tracks() {
    let source_db = create_test_db().await;

    let aid: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('The Beatles') RETURNING id")
        .fetch_one(&source_db).await.unwrap();
    let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Song A', 'ISRC001') RETURNING id")
        .fetch_one(&source_db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(tid).bind(aid).execute(&source_db).await.unwrap();

    let pid: i64 = sqlx::query_scalar("INSERT INTO playlists (account_id, name, description) VALUES (1, 'Rock Classics', 'Best hits') RETURNING id")
        .fetch_one(&source_db).await.unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
        .bind(pid).bind(tid).execute(&source_db).await.unwrap();

    let source_app = create_test_app(source_db);
    let dest = get_test_backup_path("test_backup_playlist");
    let export_res = export_library(source_app.state::<AppState>(), Some(dest)).await.unwrap();
    assert_eq!(export_res.playlists_count, 1);

    let target_db = create_test_db().await;
    let target_app = create_test_app(target_db.clone());

    let import_res = import_library(target_app.state::<AppState>(), export_res.file_path.clone(), None).await.unwrap();
    assert_eq!(import_res.playlists_imported, 1);
    assert_eq!(import_res.tracks_imported, 1);

    let pt_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks").fetch_one(&target_db).await.unwrap();
    assert_eq!(pt_count.0, 1, "Playlist track link must be restored");

    let _ = std::fs::remove_file(&export_res.file_path);
}
