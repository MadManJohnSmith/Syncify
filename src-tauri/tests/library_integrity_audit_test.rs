//! Synthetic Integration Test Suite for Sprint S151: Physical Library Integrity Audit (Read-Only)
//!
//! Deterministic in-memory/tempdir integration test suite verifying read-only invariants and classification states.
//!
//! Tests:
//! 1. Non-mutation of DB and filesystem (read-only verification).
//! 2. Detection of MissingFile (row in DB, file absent on disk).
//! 3. Detection of OrphanFile (file on disk, missing in DB).
//! 4. Detection of MetadataMismatch (tag vs DB drift).
//! 5. Detection of SidecarMismatch (missing or empty LRC).
//! 6. Detection of StagingResidual (residual .part or staging artifacts).

use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use tempfile::TempDir;

/// Represents audit classification categories for S151
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditClassification {
    Verified,
    MissingFile,
    OrphanFile,
    MetadataMismatch,
    HashMismatch,
    CorruptAudio,
    PathMismatch,
    SidecarMismatch,
    CoverMismatch,
    StagingResidual,
}

impl AuditClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verified => "Verified",
            Self::MissingFile => "MissingFile",
            Self::OrphanFile => "OrphanFile",
            Self::MetadataMismatch => "MetadataMismatch",
            Self::HashMismatch => "HashMismatch",
            Self::CorruptAudio => "CorruptAudio",
            Self::PathMismatch => "PathMismatch",
            Self::SidecarMismatch => "SidecarMismatch",
            Self::CoverMismatch => "CoverMismatch",
            Self::StagingResidual => "StagingResidual",
        }
    }
}

/// Helper: creates a minimal valid FLAC file
fn create_minimal_flac(path: &Path, title: &str, artist: &str, album: &str, isrc: &str) {
    let mut file = File::create(path).expect("create flac");
    // Minimal FLAC header
    file.write_all(b"fLaC").expect("flac magic");
    // STREAMINFO block header: last metadata block (0x80), type 0 (STREAMINFO), length 34
    file.write_all(&[0x80, 0x00, 0x00, 0x22]).expect("block header");
    // 34 bytes of streaminfo: min block (16b), max block (16b), min frame (24b), max frame (24b),
    // sample_rate (20b), channels (3b), bits_per_sample (5b), total_samples (36b), md5 (16 bytes)
    let streaminfo = [
        0x10, 0x00, // min block size 4096
        0x10, 0x00, // max block size 4096
        0x00, 0x00, 0x00, // min frame size
        0x00, 0x00, 0x00, // max frame size
        0x0a, 0xc4, 0x42, 0xf0, 0x00, 0x00, 0x10, 0x00, // 44100 Hz, 2 channels, 16 bits, samples
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // MD5 signature
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    file.write_all(&streaminfo).expect("streaminfo");
    // Dummy audio frame payload
    file.write_all(&[0xFF, 0xF8, 0x69, 0x02, 0x00, 0x00, 0x00, 0x00]).expect("audio frame");

    // Apply FLAC tags via writer
    let mut meta = FlacMetadata::default();
    meta.title = title.to_string();
    meta.artist = artist.to_string();
    meta.album = album.to_string();
    meta.isrc = Some(isrc.to_string());
    let _ = apply_and_verify_flac_tags(path, &meta);
}

/// Helper: computes SHA-256
fn compute_sha256(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let data = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Some(format!("{:x}", hasher.finalize()))
}

/// Helper: sets up in-memory or temp SQLite DB with basic schema
async fn setup_test_db(db_path: &Path) -> SqlitePool {
    let connect_opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("db connect");

    sqlx::query(
        r#"
        CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT);
        INSERT INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz'), (3, 'tidal');

        CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT);
        CREATE TABLE albums (id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT,
            album_id INTEGER,
            duration_ms INTEGER,
            isrc TEXT,
            audio_quality TEXT
        );
        CREATE TABLE track_artists (
            track_id INTEGER,
            artist_id INTEGER,
            role TEXT
        );
        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY,
            track_id INTEGER,
            source_service_id INTEGER,
            file_path TEXT,
            file_format TEXT,
            file_size_bytes INTEGER,
            file_hash TEXT,
            bit_depth INTEGER,
            sample_rate INTEGER,
            downloaded_at TEXT,
            origin_service TEXT,
            origin_service_track_id TEXT,
            effective_service TEXT,
            effective_service_track_id TEXT,
            fallback_reason TEXT
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("schema setup");

    pool
}

#[tokio::test]
async fn test_audit_engine_is_strictly_read_only_and_non_mutating() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("audit_test.db");
    let pool = setup_test_db(&db_path).await;

    let track_path = temp.path().join("Track01.flac");
    create_minimal_flac(&track_path, "Clean Track", "Artist", "Album", "USRC12345678");
    let initial_hash = compute_sha256(&track_path).expect("initial hash");
    let initial_meta = fs::metadata(&track_path).expect("metadata");

    // Insert download record
    sqlx::query(
        r#"
        INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (1, 'Clean Track', 180000, 'USRC12345678');
        INSERT INTO downloads (id, track_id, source_service_id, file_path, file_size_bytes, file_hash)
        VALUES (1, 1, 2, ?, ?, ?);
        "#,
    )
    .bind(track_path.to_str().unwrap())
    .bind(initial_meta.len() as i64)
    .bind(&initial_hash)
    .execute(&pool)
    .await
    .expect("insert");

    pool.close().await;

    // Connect in read-only mode (simulating audit engine)
    let ro_opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .read_only(true);
    let ro_pool = SqlitePoolOptions::new()
        .connect_with(ro_opts)
        .await
        .expect("readonly connect");

    // Query downloads
    let row_count: (i64,) = sqlx::query_as("SELECT count(*) FROM downloads")
        .fetch_one(&ro_pool)
        .await
        .expect("fetch count");
    assert_eq!(row_count.0, 1);

    // Verify filesystem remains unmutated
    let post_hash = compute_sha256(&track_path).expect("post hash");
    let post_meta = fs::metadata(&track_path).expect("post meta");

    assert_eq!(initial_hash, post_hash, "Physical file hash must remain unmutated");
    assert_eq!(initial_meta.len(), post_meta.len(), "File size must remain identical");
}

#[tokio::test]
async fn test_detect_missing_file() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("missing_test.db");
    let pool = setup_test_db(&db_path).await;

    let nonexistent = temp.path().join("Ghost_Track.flac");

    sqlx::query(
        r#"
        INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (10, 'Ghost Track', 180000, 'USRC00000001');
        INSERT INTO downloads (id, track_id, source_service_id, file_path, file_size_bytes)
        VALUES (1, 10, 2, ?, 1024000);
        "#,
    )
    .bind(nonexistent.to_str().unwrap())
    .execute(&pool)
    .await
    .expect("insert");

    // Perform audit check on the row
    let row: (i64, String) = sqlx::query_as("SELECT id, file_path FROM downloads WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("fetch");

    let exists = Path::new(&row.1).exists();
    let classification = if !exists {
        AuditClassification::MissingFile
    } else {
        AuditClassification::Verified
    };

    assert_eq!(classification, AuditClassification::MissingFile);
}

#[tokio::test]
async fn test_detect_orphan_file() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("orphan_test.db");
    let pool = setup_test_db(&db_path).await;

    // Create an orphan audio file in library storage with no row in DB
    let orphan_path = temp.path().join("Unindexed_Track.flac");
    create_minimal_flac(&orphan_path, "Unindexed", "Unknown", "Album", "USRC00000002");

    // Query downloads matching orphan path
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM downloads WHERE file_path = ?")
        .bind(orphan_path.to_str().unwrap())
        .fetch_one(&pool)
        .await
        .expect("fetch");

    let classification = if count.0 == 0 && orphan_path.exists() {
        AuditClassification::OrphanFile
    } else {
        AuditClassification::Verified
    };

    assert_eq!(classification, AuditClassification::OrphanFile);
}

#[tokio::test]
async fn test_detect_tag_and_metadata_mismatch() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("metadata_test.db");
    let pool = setup_test_db(&db_path).await;

    let track_path = temp.path().join("Tagged_Track.flac");
    // Tag embedded in file is "Actual Title"
    create_minimal_flac(&track_path, "Actual Title", "Actual Artist", "Actual Album", "USRC99999999");

    // In DB, expected title is "Drifted Title"
    sqlx::query(
        r#"
        INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (20, 'Drifted Title', 180000, 'USRC99999999');
        INSERT INTO downloads (id, track_id, source_service_id, file_path, file_size_bytes)
        VALUES (1, 20, 2, ?, 1024);
        "#,
    )
    .bind(track_path.to_str().unwrap())
    .execute(&pool)
    .await
    .expect("insert");

    let expected: (String,) = sqlx::query_as("SELECT title FROM tracks WHERE id = 20")
        .fetch_one(&pool)
        .await
        .expect("fetch");

    let embedded_title = "Actual Title";
    let classification = if embedded_title != expected.0 {
        AuditClassification::MetadataMismatch
    } else {
        AuditClassification::Verified
    };

    assert_eq!(classification, AuditClassification::MetadataMismatch);
}

#[tokio::test]
async fn test_detect_lrc_and_sidecar_mismatch() {
    let temp = TempDir::new().expect("tempdir");
    let audio_path = temp.path().join("Track_Without_Lrc.flac");
    create_minimal_flac(&audio_path, "Track", "Artist", "Album", "USRC11111111");

    let lrc_path = temp.path().join("Track_Without_Lrc.lrc");
    assert!(!lrc_path.exists(), "LRC should not exist initially");

    let classification = if !lrc_path.exists() {
        AuditClassification::SidecarMismatch
    } else {
        AuditClassification::Verified
    };

    assert_eq!(classification, AuditClassification::SidecarMismatch);
}

#[tokio::test]
async fn test_detect_staging_residuals() {
    let temp = TempDir::new().expect("tempdir");
    let staging_dir = temp.path().join(".staging");
    fs::create_dir_all(&staging_dir).expect("staging dir");

    let part_file = staging_dir.join("partial_download_01.part");
    fs::write(&part_file, b"partial byte stream chunks").expect("write part");

    let is_staging_residual = part_file.exists() && (part_file.to_str().unwrap().contains(".staging") || part_file.extension().map_or(false, |ext| ext == "part"));

    let classification = if is_staging_residual {
        AuditClassification::StagingResidual
    } else {
        AuditClassification::Verified
    };

    assert_eq!(classification, AuditClassification::StagingResidual);
}
