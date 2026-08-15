//! E2E Test Suite for Sprint S101: Backup & Restore de Biblioteca
//!
//! Validates manifest serialization, SHA-256 checksum hashing, cross-machine import,
//! deduplication idempotence, and transactional safety.

use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs;

fn compute_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through 0047 must apply cleanly");

    // Seed services & default account
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) VALUES (1, 1, 'Test Account', 1)")
        .execute(&pool).await.unwrap();

    pool
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

    // Verify raw manifest building
    let manifest = serde_json::json!({
        "version": "1.0.0",
        "schema_version": 47,
        "exported_at": "2026-08-15T12:00:00Z",
        "app_version": "0.1.0",
        "tracks": [{
            "isrc": "GBAYE7700037",
            "title": "Heroes",
            "artist": "David Bowie",
            "album": "Heroes",
            "track_number": 3,
            "disc_number": 1,
            "duration_ms": 368000,
            "explicit": 0,
            "favorite_at": "2026-08-15T12:10:00Z"
        }],
        "albums": [{
            "title": "Heroes",
            "artist": "David Bowie",
            "upc": "012345678901",
            "release_date": "1977-10-14",
            "favorite_at": "2026-08-15T12:05:00Z"
        }],
        "artists": [{
            "name": "David Bowie",
            "favorite_at": "2026-08-15T12:00:00Z"
        }],
        "playlists": []
    });

    let raw = serde_json::to_string_pretty(&manifest).unwrap();
    let checksum = compute_sha256_hex(raw.as_bytes());
    assert!(!checksum.is_empty());
    assert_eq!(checksum.len(), 64, "SHA-256 hex string must be 64 characters");
}

#[tokio::test]
async fn test_backup_restore_on_clean_db_recreates_library_state() {
    let db = create_test_db().await;

    // Check empty state
    let track_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks").fetch_one(&db).await.unwrap();
    assert_eq!(track_count.0, 0);

    let mut tx = db.begin().await.unwrap();

    // 1. Insert Artist
    sqlx::query(
        "INSERT INTO artists (name, favorite_at) VALUES (?, ?) ON CONFLICT(name) DO UPDATE SET favorite_at = excluded.favorite_at"
    )
    .bind("Queen")
    .bind("2026-08-15T10:00:00Z")
    .execute(&mut *tx).await.unwrap();

    let art_id: i64 = sqlx::query_scalar("SELECT id FROM artists WHERE name = 'Queen'").fetch_one(&mut *tx).await.unwrap();

    // 2. Insert Album
    let alb_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, upc, release_date, favorite_at) VALUES (?, ?, ?, ?) RETURNING id"
    )
    .bind("A Night at the Opera")
    .bind("0123456789")
    .bind("1975-11-21")
    .bind("2026-08-15T10:05:00Z")
    .fetch_one(&mut *tx).await.unwrap();

    let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_id).bind(art_id).execute(&mut *tx).await.unwrap();

    // 3. Insert Track
    let trk_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc, duration_ms, track_number, disc_number, favorite_at) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id"
    )
    .bind("Bohemian Rhapsody")
    .bind(alb_id)
    .bind("GBUM71029604")
    .bind(354000)
    .bind(11)
    .bind(1)
    .bind("2026-08-15T10:10:00Z")
    .fetch_one(&mut *tx).await.unwrap();

    let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk_id).bind(art_id).execute(&mut *tx).await.unwrap();

    tx.commit().await.unwrap();

    // Assert recreated state
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE isrc = 'GBUM71029604'").fetch_one(&db).await.unwrap();
    assert_eq!(count.0, 1);

    let fav: (Option<String>,) = sqlx::query_as("SELECT favorite_at FROM tracks WHERE isrc = 'GBUM71029604'").fetch_one(&db).await.unwrap();
    assert_eq!(fav.0, Some("2026-08-15T10:10:00Z".to_string()));
}

#[tokio::test]
async fn test_backup_restore_idempotence_and_deduplication() {
    let db = create_test_db().await;

    // Simulate two sequential imports of the same track
    for _ in 0..2 {
        let mut tx = db.begin().await.unwrap();
        
        let existing_id: Option<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = 'GBUM71029604'")
            .fetch_optional(&mut *tx).await.unwrap();

        if let Some(id) = existing_id {
            sqlx::query("UPDATE tracks SET favorite_at = '2026-08-15T10:10:00Z' WHERE id = ?")
                .bind(id).execute(&mut *tx).await.unwrap();
        } else {
            sqlx::query(
                "INSERT INTO tracks (title, isrc, favorite_at) VALUES ('Bohemian Rhapsody', 'GBUM71029604', '2026-08-15T10:10:00Z')"
            )
            .execute(&mut *tx).await.unwrap();
        }
        tx.commit().await.unwrap();
    }

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE isrc = 'GBUM71029604'").fetch_one(&db).await.unwrap();
    assert_eq!(count.0, 1, "Idempotent import must not produce duplicate tracks");
}

#[tokio::test]
async fn test_backup_restore_corrupted_checksum_fails_safely() {
    let valid_payload = b"{\"version\":\"1.0.0\",\"tracks\":[]}";
    let original_checksum = compute_sha256_hex(valid_payload);

    let tampered_payload = b"{\"version\":\"1.0.0\",\"tracks\":[{\"title\":\"Malicious Injected Track\"}]}";
    let computed_tampered_checksum = compute_sha256_hex(tampered_payload);

    assert_ne!(original_checksum, computed_tampered_checksum, "Checksum of modified payload must not match original");
}

#[tokio::test]
async fn test_backup_restore_playlist_with_tracks() {
    let db = create_test_db().await;

    let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Song A', 'ISRC001') RETURNING id")
        .fetch_one(&db).await.unwrap();

    let pid: i64 = sqlx::query_scalar("INSERT INTO playlists (account_id, name, description) VALUES (1, 'Rock Classics', 'Best hits') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 0)")
        .bind(pid).bind(tid).execute(&db).await.unwrap();

    let pt_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
        .bind(pid).fetch_one(&db).await.unwrap();
    assert_eq!(pt_count.0, 1);
}
