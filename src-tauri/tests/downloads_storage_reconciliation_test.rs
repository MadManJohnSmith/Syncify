//! Integration Test Suite for TASK-102:
//! Downloads Storage Reconciliation and Staging Residual Purge
//!
//! Validates:
//! 1. Atomic reconciliation of physical FLAC files matching tracks by ISRC into `downloads`.
//! 2. Unambiguous fallback reconciliation by canonical Title + Artist when ISRC is absent.
//! 3. M4A / AAC storage discovery and registration with correct file_format in `downloads`.
//! 4. Automatic cleanup/purging of abandoned `.staging/*.part` residual files.
//! 5. Comprehensive integrity audit detection of orphan physical files and subsequent resolution.
//! 6. Idempotent re-execution producing 0 modifications.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use syncify_tauri_lib::commands::{
    perform_reconcile_downloads_from_storage,
    perform_run_integrity_audit,
};
use tempfile::TempDir;

async fn setup_test_schema(pool: &SqlitePool) {
    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            supports_download INTEGER DEFAULT 1
        );
        INSERT INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz'), (3, 'tidal');

        CREATE TABLE artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL
        );

        CREATE TABLE albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            artist_id INTEGER,
            cover_art_url TEXT
        );

        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            album_id INTEGER,
            isrc TEXT,
            duration_ms INTEGER,
            download_status TEXT
        );

        CREATE TABLE track_artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            artist_id INTEGER NOT NULL,
            role TEXT DEFAULT 'primary'
        );

        CREATE TABLE track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            service_id INTEGER NOT NULL,
            service_track_id TEXT,
            available INTEGER DEFAULT 1
        );

        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id) ON DELETE SET NULL,
            source_service_id INTEGER REFERENCES services(id),
            file_path TEXT NOT NULL,
            file_format TEXT,
            file_size_bytes INTEGER,
            file_hash TEXT,
            bit_depth INTEGER,
            sample_rate INTEGER,
            metadata_completeness INTEGER DEFAULT 0,
            downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
            only_available_on TEXT,
            not_streaming INTEGER DEFAULT 0,
            effective_service TEXT,
            effective_service_track_id TEXT,
            match_method TEXT
        );

        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'queued'
        );

        CREATE TABLE folder_settings (
            id INTEGER PRIMARY KEY,
            base_folder TEXT NOT NULL
        );
        "#
    )
    .execute(pool)
    .await
    .expect("Failed to initialize test schema");
}

fn create_test_flac(path: &Path, isrc: Option<&str>, title: &str, artist: &str) {
    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC");
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]); // STREAMINFO header
    data.extend_from_slice(&[0u8; 34]); // STREAMINFO body (34 bytes)
    data.extend_from_slice(&[0x81, 0x00, 0x00, 0x00]); // PADDING header
    data.extend(vec![0xCC; 2048]);
    std::fs::write(path, &data).expect("Failed to write initial FLAC frame");

    let meta = syncify_flac_writer::FlacMetadata {
        title: title.to_string(),
        artist: artist.to_string(),
        album: "Test Album".to_string(),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        isrc: isrc.map(|s| s.to_string()),
        bit_depth: Some(24),
        sample_rate: Some(48000.0),
        audio_source: Some("Qobuz".to_string()),
        ..Default::default()
    };
    let _ = syncify_flac_writer::apply_and_verify_flac_tags(path, &meta);
}

fn create_test_m4a(path: &PathBuf, isrc: Option<&str>, title: &str, artist: &str) {
    // Generate minimal PCM wav and convert to aac/m4a using ffmpeg if available
    let temp_wav = path.with_extension("wav");
    let mut wav_bytes = Vec::new();
    let num_samples = 44100 / 2; // 0.5s
    let sample_rate = 44100u32;
    let byte_rate = sample_rate * 2;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&((36 + num_samples * 2) as u32).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&2u16.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&((num_samples * 2) as u32).to_le_bytes());
    wav_bytes.extend(vec![0u8; (num_samples * 2) as usize]);

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp wav");

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "aac",
            "-b:a", "128k",
            path.to_str().unwrap(),
        ])
        .output();

    let _ = std::fs::remove_file(&temp_wav);

    if status.is_ok() && path.exists() {
        let meta = syncify_tauri_lib::services::mp4_writer::Mp4Metadata {
            title: title.to_string(),
            artist: artist.to_string(),
            album: "Test M4A Album".to_string(),
            isrc: isrc.map(|s| s.to_string()),
            audio_source: Some("Tidal".to_string()),
            ..Default::default()
        };
        let _ = syncify_tauri_lib::services::mp4_writer::apply_and_verify_mp4_tags(path, &meta);
    }
}

#[tokio::test]
async fn test_reconcile_flac_exact_isrc_and_purge_staging() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    let staging_dir = music_dir.join(".staging");
    std::fs::create_dir_all(&music_dir).unwrap();
    std::fs::create_dir_all(&staging_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(music_dir_str)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Seed database with artist, album, track
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Kacey Musgraves') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Golden Hour', ?) RETURNING id")
        .bind(artist_id).fetch_one(&pool).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Slow Burn', ?, 'USUM71801234') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
        .bind(track_id).bind(artist_id).execute(&pool).await.unwrap();

    // 2. Create physical audio file on disk
    let album_dir = music_dir.join("Kacey Musgraves").join("Golden Hour");
    std::fs::create_dir_all(&album_dir).unwrap();
    let flac_path = album_dir.join("01 - Slow Burn.flac");
    create_test_flac(&flac_path, Some("USUM71801234"), "Slow Burn", "Kacey Musgraves");

    // 3. Create orphaned staging partial file
    let part_file = staging_dir.join("download_partial_xyz.part");
    std::fs::write(&part_file, b"incomplete partial chunk").unwrap();
    assert!(part_file.exists());

    // 4. Pre-condition: downloads table is completely empty
    let dl_count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(dl_count_before, 0);

    // 5. Execute storage reconciliation command
    let res = perform_reconcile_downloads_from_storage(&pool, Some(music_dir_str.to_string()))
        .await
        .expect("Storage reconciliation must succeed");

    assert_eq!(res.scanned_audio_files, 1, "Must scan 1 physical audio file");
    assert_eq!(res.relinked_downloads, 1, "Must relink 1 track into downloads");
    assert_eq!(res.purged_staging_files, 1, "Must purge 1 staging .part file");
    assert!(res.ambiguous_files.is_empty(), "No files should be ambiguous");

    // 6. Verify downloads table entry
    let row: (i64, i64, String, String, i64, String) = sqlx::query_as(
        "SELECT track_id, source_service_id, file_path, file_format, file_size_bytes, file_hash FROM downloads WHERE track_id = ?"
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, track_id);
    assert_eq!(row.1, 2); // Qobuz service_id
    assert_eq!(row.2, flac_path.to_str().unwrap());
    assert_eq!(row.3, "FLAC");
    assert!(row.4 > 0, "file_size_bytes must be positive");
    assert_eq!(row.5.len(), 64, "SHA-256 hash must be 64 hexadecimal characters");

    // 7. Verify disk state: .part purged, flac preserved
    assert!(!part_file.exists(), ".part staging file must be purged");
    assert!(flac_path.exists(), "Physical FLAC file must remain on disk");
}

#[tokio::test]
async fn test_reconcile_flac_title_artist_fallback() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(music_dir_str)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Seed track without ISRC
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('M83') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Hurry Up', ?) RETURNING id")
        .bind(artist_id).fetch_one(&pool).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Midnight City', ?, NULL) RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
        .bind(track_id).bind(artist_id).execute(&pool).await.unwrap();

    // 2. Create physical FLAC with Title + Artist only (no ISRC)
    let flac_path = music_dir.join("01 - Midnight City.flac");
    create_test_flac(&flac_path, None, "Midnight City", "M83");

    // 3. Execute reconciliation
    let res = perform_reconcile_downloads_from_storage(&pool, Some(music_dir_str.to_string()))
        .await
        .unwrap();

    assert_eq!(res.relinked_downloads, 1, "Must match via unambiguous title+artist");

    let registered_track_id: i64 = sqlx::query_scalar("SELECT track_id FROM downloads WHERE file_path = ?")
        .bind(flac_path.to_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(registered_track_id, track_id);
}

#[tokio::test]
async fn test_reconcile_m4a_by_isrc() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(music_dir_str)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Seed track in DB
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Dua Lipa') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Future Nostalgia', ?) RETURNING id")
        .bind(artist_id).fetch_one(&pool).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Levitating', ?, 'GBAYE2000001') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
        .bind(track_id).bind(artist_id).execute(&pool).await.unwrap();

    // 2. Create physical M4A file
    let m4a_path = music_dir.join("05 - Levitating.m4a");
    create_test_m4a(&m4a_path, Some("GBAYE2000001"), "Levitating", "Dua Lipa");

    if !m4a_path.exists() {
        eprintln!("ffmpeg not generating M4A in test environment, skipping M4A test");
        return;
    }

    // 3. Execute reconciliation
    let res = perform_reconcile_downloads_from_storage(&pool, Some(music_dir_str.to_string()))
        .await
        .unwrap();

    assert_eq!(res.relinked_downloads, 1, "Must relink M4A file");

    let (file_format, relinked_tid): (String, i64) = sqlx::query_as(
        "SELECT file_format, track_id FROM downloads WHERE file_path = ?"
    )
    .bind(m4a_path.to_str().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(file_format, "M4A");
    assert_eq!(relinked_tid, track_id);
}

#[tokio::test]
async fn test_integrity_audit_detects_orphans_and_resolves() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(music_dir_str)
        .execute(&pool)
        .await
        .unwrap();

    // Seed track
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Radiohead') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('In Rainbows', ?) RETURNING id")
        .bind(artist_id).fetch_one(&pool).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('15 Step', ?, 'GBAYE0700101') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
        .bind(track_id).bind(artist_id).execute(&pool).await.unwrap();

    // Create physical FLAC on disk without downloads row
    let flac_path = music_dir.join("01 - 15 Step.flac");
    create_test_flac(&flac_path, Some("GBAYE0700101"), "15 Step", "Radiohead");

    // 1. Audit before reconciliation: should detect 1 orphan file
    let audit_before = perform_run_integrity_audit(&pool, Some(music_dir_str.to_string()))
        .await
        .expect("Integrity audit must run");

    assert_eq!(audit_before.orphan_files.len(), 1, "Must report 1 orphan physical audio file");
    assert_eq!(audit_before.orphan_files[0], flac_path.to_str().unwrap());
    assert!(!audit_before.is_healthy, "Audit must report unhealthy when orphan files exist");

    // 2. Perform reconciliation
    let rec_res = perform_reconcile_downloads_from_storage(&pool, Some(music_dir_str.to_string()))
        .await
        .expect("Reconciliation must succeed");
    assert_eq!(rec_res.relinked_downloads, 1);

    // 3. Audit after reconciliation: orphan should be resolved
    let audit_after = perform_run_integrity_audit(&pool, Some(music_dir_str.to_string()))
        .await
        .expect("Integrity audit must run");

    assert_eq!(audit_after.orphan_files.len(), 0, "No orphan files should remain after reconciliation");
    assert_eq!(audit_after.verified_files, 1, "The relinked file must now be verified");
    assert!(audit_after.is_healthy, "Audit must be healthy after reconciliation");

    // 4. Idempotency: Second reconciliation does nothing
    let rec_res_2 = perform_reconcile_downloads_from_storage(&pool, Some(music_dir_str.to_string()))
        .await
        .unwrap();
    assert_eq!(rec_res_2.relinked_downloads, 0, "Second run must relink 0 records");
    assert_eq!(rec_res_2.purged_staging_files, 0, "Second run must purge 0 files");
}
