//! The batch manifest is the artifact the user audits or moves the batch with, so
//! it has to describe the bytes that actually landed on disk.
//!
//! It used to hardcode `FLAC` / `Lossless` for every successful row, which meant a
//! transfer that arrived as AAC was filed as lossless FLAC — the same contradiction
//! R1 reported from the other direction.

use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use syncify_tauri_lib::services::ManifestWriter;
use tempfile::TempDir;

async fn seed_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");

    sqlx::query(
        r#"
        CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT);
        CREATE TABLE albums (id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT, isrc TEXT, album_id INTEGER, artist_id INTEGER);
        CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER);
        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY,
            track_id INTEGER,
            service_name TEXT,
            service_track_id TEXT,
            target_title TEXT,
            target_artist TEXT,
            target_album TEXT,
            target_isrc TEXT,
            status TEXT,
            error_message TEXT,
            quality_preference TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT
        );
        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY,
            track_id INTEGER UNIQUE,
            file_path TEXT,
            file_format TEXT,
            bit_depth INTEGER,
            sample_rate INTEGER,
            file_size_bytes INTEGER,
            downloaded_at TEXT
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("schema");

    pool
}

fn write_m4a(path: &Path) {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&[0u8, 0u8, 0u8, 0x18]); // box size
    bytes.extend_from_slice(b"ftyp");
    bytes.extend_from_slice(b"M4A ");
    bytes.extend_from_slice(&[0u8; 8]);
    std::fs::write(path, bytes).expect("write m4a");
}

fn write_flac(path: &Path) {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"fLaC");
    bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // last STREAMINFO block, 34 bytes
    bytes.extend_from_slice(&4096u16.to_be_bytes()); // min block size
    bytes.extend_from_slice(&4096u16.to_be_bytes()); // max block size
    bytes.extend_from_slice(&[0u8; 3]); // min frame size
    bytes.extend_from_slice(&[0u8; 3]); // max frame size
    let packed: u64 = (44100u64 << 44) | (1u64 << 41) | (15u64 << 36);
    bytes.extend_from_slice(&packed.to_be_bytes()); // rate | channels-1 | bps-1 | samples
    bytes.extend_from_slice(&[0u8; 16]); // MD5 signature
    std::fs::write(path, bytes).expect("write flac");
}

#[tokio::test]
async fn a_lossy_arrival_is_manifested_as_lossy_not_as_flac() {
    let pool = seed_db().await;
    let out_dir = TempDir::new().unwrap();
    let audio = out_dir.path().join("01 - Arrived.m4a");
    write_m4a(&audio);

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, status, quality_preference)
        VALUES (1, 201, 'tidal', '777888', 'Arrived', 'Artist', 'Album', 'complete', '24-192');
        "#,
    )
    .execute(&pool)
    .await
    .expect("queue row");

    // The ledger still claims FLAC/24/96k — the manifest must not inherit that claim.
    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes) VALUES (201, ?, 'FLAC', 24, 96000, 1024)",
    )
    .bind(audio.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .expect("downloads row");

    let manifest = ManifestWriter::generate_and_save_manifest(&pool, out_dir.path())
        .await
        .expect("manifest");

    let entry = manifest
        .entries
        .iter()
        .find(|e| e.track_id == Some(201))
        .expect("entry for the completed track");

    assert_eq!(
        entry.format_obtained.as_deref(),
        Some("AAC"),
        "the manifest filed a physically AAC file as FLAC"
    );
    assert_eq!(entry.quality_class_obtained.as_deref(), Some("Lossy"));
    assert_eq!(entry.extension.as_deref(), Some("m4a"));
    assert_eq!(
        entry.flac_validation, "None",
        "FLAC validation cannot be claimed for a non-FLAC container"
    );
    // Measured off the file, not copied from the optimistic ledger row.
    assert_eq!(entry.bit_depth, Some(16));
    assert_eq!(entry.sample_rate, Some(44100));
}

#[tokio::test]
async fn a_lossless_arrival_is_manifested_as_flac_and_validated() {
    let pool = seed_db().await;
    let out_dir = TempDir::new().unwrap();
    let audio = out_dir.path().join("01 - Lossless.flac");
    write_flac(&audio);

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, status, quality_preference)
        VALUES (1, 301, 'tidal', '777999', 'Lossless', 'Artist', 'Album', 'complete', '16-44');
        "#,
    )
    .execute(&pool)
    .await
    .expect("queue row");

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes) VALUES (301, ?, 'FLAC', 16, 44100, 1024)",
    )
    .bind(audio.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .expect("downloads row");

    let manifest = ManifestWriter::generate_and_save_manifest(&pool, out_dir.path())
        .await
        .expect("manifest");

    let entry = manifest
        .entries
        .iter()
        .find(|e| e.track_id == Some(301))
        .expect("entry for the completed track");

    assert_eq!(entry.format_obtained.as_deref(), Some("FLAC"));
    assert_eq!(entry.quality_class_obtained.as_deref(), Some("Lossless"));
    assert_eq!(entry.extension.as_deref(), Some("flac"));
    assert_eq!(entry.flac_validation, "Valid");
    assert_eq!(entry.bit_depth, Some(16));
    assert_eq!(entry.sample_rate, Some(44100));
}
