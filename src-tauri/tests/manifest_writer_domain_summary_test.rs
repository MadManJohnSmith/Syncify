//! TASK-7.1 / CR-4: the batch manifest is produced by the domain summary
//! (`FavoritesBatchSummary::to_batch_manifest`), not assembled field by field in
//! `services::manifest_writer`.
//!
//! The written `manifest.json` must therefore satisfy, for every reconciled queue
//! row, the same mapping the domain defines — counters derived from the queue status
//! and per-entry fields derived from the entries themselves.

use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use syncify_core_domain::{BatchDownloadManifest, FavoritesBatchSummary};
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

#[tokio::test]
async fn manifest_totals_follow_the_domain_summary_mapping() {
    let pool = seed_db().await;
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();
    let audio = out_dir.join("01 - Done.flac");
    tokio::fs::write(&audio, b"FLAC DATA").await.unwrap();

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, status, quality_preference)
        VALUES (1, 101, 'qobuz', '999111', 'Done', 'Artist', 'Album', 'complete', '24-96');
        INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes)
        VALUES (101, ?, 'FLAC', 24, 96000, 1024);
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, status, error_message, quality_preference)
        VALUES (2, 102, 'qobuz', '999222', 'Nope', 'failed', 'NetworkExhausted: retries spent', '16-44');
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, status, quality_preference)
        VALUES (3, 103, 'qobuz', '999333', 'Skipped', 'skipped', '16-44');
        "#,
    )
    .bind(audio.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let manifest = ManifestWriter::generate_and_save_manifest(&pool, Path::new(out_dir))
        .await
        .expect("manifest");

    // The manifest on disk must round-trip through the domain type...
    let on_disk = tokio::fs::read_to_string(out_dir.join("manifest.json"))
        .await
        .unwrap();
    let parsed: BatchDownloadManifest =
        serde_json::from_str(&on_disk).expect("valid manifest json");
    assert_eq!(parsed.entries, manifest.entries);

    // ...and match the mapping `FavoritesBatchSummary::to_batch_manifest` defines.
    let summary = FavoritesBatchSummary {
        requested: manifest.entries.len(),
        received: manifest.entries.len(),
        deduplicated: 0,
        skipped_existing: 1,
        succeeded: 1,
        failed: 1,
        enriched: manifest
            .entries
            .iter()
            .filter(|e| e.enrichment_result == "Success")
            .count(),
        validated: manifest
            .entries
            .iter()
            .filter(|e| e.audio_validation == "Valid")
            .count(),
        output_files: manifest
            .entries
            .iter()
            .filter(|e| e.final_path.is_some())
            .count(),
        manifest: manifest.entries.clone(),
    };
    assert_eq!(
        manifest,
        summary.to_batch_manifest(manifest.generated_at.clone())
    );

    assert_eq!(manifest.total_requested, 3);
    assert_eq!(manifest.total_succeeded, 1);
    assert_eq!(manifest.total_failed, 1);
    assert_eq!(manifest.total_skipped, 1);
    // Only the completed row has a promoted file.
    assert_eq!(
        manifest
            .entries
            .iter()
            .filter(|e| e.final_path.is_some())
            .count(),
        manifest.total_succeeded
    );
}

#[tokio::test]
async fn empty_queue_produces_an_empty_but_valid_manifest() {
    let pool = seed_db().await;
    let temp_dir = TempDir::new().unwrap();

    let manifest = ManifestWriter::generate_and_save_manifest(&pool, Path::new(temp_dir.path()))
        .await
        .expect("manifest");

    assert_eq!(manifest.total_requested, 0);
    assert_eq!(manifest.total_succeeded, 0);
    assert_eq!(manifest.total_failed, 0);
    assert_eq!(manifest.total_skipped, 0);
    assert!(manifest.entries.is_empty());
    assert!(
        !manifest.generated_at.is_empty(),
        "an empty batch still records when it was generated"
    );
}
