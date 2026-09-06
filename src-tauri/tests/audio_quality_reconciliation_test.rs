//! TASK-72: Audio Quality Reconciliation and Real Disk Quality Tracking Tests
//!
//! Tests:
//! 1. Physical inspection of FLAC headers via `inspect_physical_audio_file`:
//!    - Standard CD quality (16-bit / 44.1kHz) -> AudioTier::Lossless
//!    - Hi-Res quality (24-bit / 96kHz) -> AudioTier::HiRes
//!    - Hi-Res quality (24-bit / 44.1kHz) -> AudioTier::HiRes
//!    - Hi-Res quality (16-bit / 96kHz) -> AudioTier::HiRes
//! 2. DownloadOrchestrator physical quality reconciliation:
//!    - Shortfall detection: Hi-Res requested, but physical file verified as 16-bit/44.1kHz CD quality
//!      -> Sets QualityDecisionKind::CompletedWithQualityShortfall, quality_fallback_used = true.
//!    - Exact Hi-Res: Hi-Res requested and verified 24-bit/96kHz on disk
//!      -> Sets QualityDecisionKind::CompletedExactQuality, quality_fallback_used = false.
//! 3. Worker mark_complete end-to-end database synchronization:
//!    - Shortfall scenario: Updates `tracks.audio_quality` to 'lossless' based on true disk reality,
//!      records exact bit_depth (16) and sample_rate (44100) in `downloads`, and flags `CompletedWithQualityShortfall`.
//!    - Hi-Res promotion scenario: Updates `tracks.audio_quality` to 'hires', records 24-bit/96kHz in `downloads`.

use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use syncify_core_domain::quality::{AudioTier, QualityDecisionKind};
use syncify_tauri_lib::download::audio_inspector::inspect_physical_audio_file;
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::{DownloadRequest, DownloadResult};
use syncify_tauri_lib::worker::{DownloadWorker, DownloadWorkerState};
use tempfile::TempDir;

/// Helper to write a bit-perfect synthetic FLAC header with custom STREAMINFO parameters
fn create_synthetic_flac(path: &Path, bit_depth: u8, sample_rate: u32, channels: u8) {
    let mut data = Vec::new();
    // 1. "fLaC" magic marker
    data.extend_from_slice(b"fLaC");

    // 2. METADATA_BLOCK_HEADER for STREAMINFO (type 0, is_last = 0, length = 34)
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]);

    // 3. STREAMINFO payload (34 bytes)
    data.extend_from_slice(&[0x10, 0x00]); // min_block_size 4096
    data.extend_from_slice(&[0x10, 0x00]); // max_block_size 4096
    data.extend_from_slice(&[0x00, 0x00, 0x0E]); // min_frame_size
    data.extend_from_slice(&[0x00, 0x38, 0xA4]); // max_frame_size

    let sr_hi = ((sample_rate >> 4) & 0xFFFF) as u16;
    let sr_lo = (sample_rate & 0x0F) as u8;
    data.extend_from_slice(&sr_hi.to_be_bytes());

    let ch_bits = (channels - 1) & 0x07;
    let bps_bits = (bit_depth - 1) & 0x1F;
    let byte12 = (sr_lo << 4) | (ch_bits << 1) | ((bps_bits >> 4) & 0x01);
    let byte13 = (bps_bits & 0x0F) << 4;
    data.push(byte12);
    data.push(byte13);

    // total_samples low 32 bits (882000 samples)
    data.extend_from_slice(&[0x00, 0x0D, 0x75, 0x50]);

    // md5 signature (16 bytes)
    data.extend_from_slice(&[0u8; 16]);

    // 4. METADATA_BLOCK_HEADER for VORBIS_COMMENT (type 4, is_last = 1, length = 8)
    data.extend_from_slice(&[0x84, 0x00, 0x00, 0x08]);
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // vendor string length 0
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // user comment count 0

    std::fs::write(path, data).expect("Failed to write synthetic FLAC");
}

#[test]
fn test_physical_flac_header_parsing_lossless_cd() {
    let dir = TempDir::new().unwrap();
    let flac_path = dir.path().join("cd_quality.flac");
    create_synthetic_flac(&flac_path, 16, 44100, 2);

    let info = inspect_physical_audio_file(&flac_path).expect("Must parse CD quality FLAC");
    assert_eq!(info.format, "FLAC");
    assert_eq!(info.bit_depth, 16);
    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.channels, 2);
    assert_eq!(info.classify_tier(), AudioTier::Lossless);
    assert_eq!(info.quality_string(), "FLAC 16-bit / 44.1kHz");
}

#[test]
fn test_physical_flac_header_parsing_hires_24_96() {
    let dir = TempDir::new().unwrap();
    let flac_path = dir.path().join("hires_96.flac");
    create_synthetic_flac(&flac_path, 24, 96000, 2);

    let info = inspect_physical_audio_file(&flac_path).expect("Must parse 24/96 HiRes FLAC");
    assert_eq!(info.format, "FLAC");
    assert_eq!(info.bit_depth, 24);
    assert_eq!(info.sample_rate, 96000);
    assert_eq!(info.channels, 2);
    assert_eq!(info.classify_tier(), AudioTier::HiRes);
    assert_eq!(info.quality_string(), "FLAC 24-bit / 96.0kHz");
}

#[test]
fn test_physical_flac_header_parsing_hires_24_44() {
    let dir = TempDir::new().unwrap();
    let flac_path = dir.path().join("hires_24_44.flac");
    create_synthetic_flac(&flac_path, 24, 44100, 2);

    let info = inspect_physical_audio_file(&flac_path).expect("Must parse 24/44.1 HiRes FLAC");
    assert_eq!(info.bit_depth, 24);
    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.classify_tier(), AudioTier::HiRes);
}

#[test]
fn test_physical_flac_header_parsing_hires_16_96() {
    let dir = TempDir::new().unwrap();
    let flac_path = dir.path().join("hires_16_96.flac");
    create_synthetic_flac(&flac_path, 16, 96000, 2);

    let info = inspect_physical_audio_file(&flac_path).expect("Must parse 16/96 HiRes FLAC");
    assert_eq!(info.bit_depth, 16);
    assert_eq!(info.sample_rate, 96000);
    assert_eq!(info.classify_tier(), AudioTier::HiRes);
}

#[test]
fn test_orchestrator_quality_reconciliation_exact_hires() {
    let dir = TempDir::new().unwrap();
    let flac_path = dir.path().join("orchestrator_hires.flac");
    create_synthetic_flac(&flac_path, 24, 96000, 2);

    let req = DownloadRequest {
        item_id: "test-1".to_string(),
        track_name: "Hi-Res Anthem".to_string(),
        artist_name: "Audiophile Artist".to_string(),
        album_name: "Master Album".to_string(),
        quality: "hires".to_string(),
        strict_quality: false,
        allow_fallback: true,
        ..Default::default()
    };

    let mut res = DownloadResult {
        file_path: flac_path.to_string_lossy().to_string(),
        service: "qobuz".to_string(),
        // Initial claims before physical inspection
        bit_depth: 16,
        sample_rate: 44100,
        ..Default::default()
    };

    DownloadOrchestrator::reconcile_physical_audio_quality(&mut res, &req);

    assert_eq!(res.bit_depth, 24, "Must reconcile true bit depth 24 from physical FLAC");
    assert_eq!(res.sample_rate, 96000, "Must reconcile true sample rate 96000 from physical FLAC");
    let qd = res.quality_decision.expect("Quality decision must be evaluated");
    assert_eq!(qd.decision, QualityDecisionKind::CompletedExactQuality);
    assert!(!qd.quality_fallback_used);
    assert_eq!(qd.effective_quality, "FLAC 24-bit / 96.0kHz");
}

#[test]
fn test_orchestrator_quality_reconciliation_shortfall_detection() {
    let dir = TempDir::new().unwrap();
    let flac_path = dir.path().join("orchestrator_shortfall.flac");
    // Stream delivers standard 16-bit / 44.1kHz FLAC
    create_synthetic_flac(&flac_path, 16, 44100, 2);

    let req = DownloadRequest {
        item_id: "test-2".to_string(),
        track_name: "Shortfall Track".to_string(),
        artist_name: "Artist".to_string(),
        album_name: "Album".to_string(),
        quality: "hires".to_string(),
        strict_quality: false,
        allow_fallback: true,
        ..Default::default()
    };

    let mut res = DownloadResult {
        file_path: flac_path.to_string_lossy().to_string(),
        service: "qobuz".to_string(),
        // Claimed HiRes initially
        bit_depth: 24,
        sample_rate: 96000,
        ..Default::default()
    };

    DownloadOrchestrator::reconcile_physical_audio_quality(&mut res, &req);

    assert_eq!(res.bit_depth, 16, "Physical inspection must correct claimed 24-bit to 16-bit");
    assert_eq!(res.sample_rate, 44100, "Physical inspection must correct claimed 96kHz to 44.1kHz");
    let qd = res.quality_decision.expect("Quality decision must be evaluated");
    assert_eq!(qd.decision, QualityDecisionKind::CompletedWithQualityShortfall, "Must flag QualityShortfall");
    assert!(qd.quality_fallback_used, "quality_fallback_used must be true for shortfall");
    assert!(qd.reason.as_ref().unwrap().contains("Quality shortfall: requested Hi-Res"));
}

#[tokio::test]
async fn test_worker_mark_complete_database_reconciliation_shortfall() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("worker_reconciliation.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // Run production migrations
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must succeed");

    // Setup base records: service, artist, album, track
    sqlx::query("INSERT INTO services (id, name, supports_download, max_quality) VALUES (1, 'qobuz', 1, 'hires') ON CONFLICT(id) DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Audited Artist')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Audited Album')")
        .execute(&pool)
        .await
        .unwrap();

    // Track initially promised as 'hires' in metadata
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc, audio_quality) VALUES ('Promised HiRes Track', 1, 200000, 'USAAA2000001', 'hires') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Queue item requesting 'hires'
    let queue_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO download_queue (track_id, status, requested_quality, progress_percent)
           VALUES (?, 'downloading', 'hires', 50.0) RETURNING id"#
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Write physical 16-bit / 44.1kHz FLAC to disk (discrepancy vs promised Hi-Res)
    let flac_file = temp_dir.path().join("shortfall_disk.flac");
    create_synthetic_flac(&flac_file, 16, 44100, 2);

    let worker_state = DownloadWorkerState::new(2);
    let worker = DownloadWorker::new(pool.clone(), worker_state);

    let download_res = DownloadResult {
        file_path: flac_file.to_string_lossy().to_string(),
        bit_depth: 16,
        sample_rate: 44100,
        title: "Promised HiRes Track".to_string(),
        artist: "Audited Artist".to_string(),
        album: "Audited Album".to_string(),
        service: "qobuz".to_string(),
        ..Default::default()
    };

    worker.mark_complete(queue_id, &download_res).await;

    // 1. Verify tracks.audio_quality was synchronized to 'lossless' (reconciled against physical reality)
    let final_track_q: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(final_track_q.as_deref(), Some("lossless"), "tracks.audio_quality must match physical 16-bit FLAC reality ('lossless')");

    // 2. Verify downloads table records the physical reality and shortfall decision
    let (bd, sr, fmt, q_dec, q_fb): (Option<i32>, Option<i32>, Option<String>, Option<String>, i64) = sqlx::query_as(
        "SELECT bit_depth, sample_rate, file_format, quality_decision, quality_fallback_used FROM downloads WHERE track_id = ?"
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(bd, Some(16));
    assert_eq!(sr, Some(44100));
    assert_eq!(fmt.as_deref(), Some("FLAC"));
    assert_eq!(q_dec.as_deref(), Some("CompletedWithQualityShortfall"));
    assert_eq!(q_fb, 1, "quality_fallback_used must be 1 in downloads");

    // 3. Verify download_queue table records the shortfall
    let (q_status, q_eff_q, q_dec_queue, q_fb_queue): (String, Option<String>, Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT status, effective_quality, quality_decision, quality_fallback_used FROM download_queue WHERE id = ?"
    )
    .bind(queue_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(q_status, "complete");
    assert_eq!(q_dec_queue.as_deref(), Some("CompletedWithQualityShortfall"));
    assert_eq!(q_fb_queue, Some(1));
    assert!(q_eff_q.as_deref().unwrap().contains("16-bit / 44.1kHz"));
}

#[tokio::test]
async fn test_worker_mark_complete_database_reconciliation_hires_promotion() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("worker_hires_promotion.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must succeed");

    sqlx::query("INSERT INTO services (id, name, supports_download, max_quality) VALUES (1, 'qobuz', 1, 'hires') ON CONFLICT(id) DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Promotion Artist')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Promotion Album')")
        .execute(&pool)
        .await
        .unwrap();

    // Track was originally 'lossless'
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc, audio_quality) VALUES ('Real HiRes Track', 1, 250000, 'USAAA2000002', 'lossless') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let queue_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO download_queue (track_id, status, requested_quality, progress_percent)
           VALUES (?, 'downloading', 'hires', 50.0) RETURNING id"#
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Write true 24-bit / 96kHz FLAC to disk
    let flac_file = temp_dir.path().join("real_hires_disk.flac");
    create_synthetic_flac(&flac_file, 24, 96000, 2);

    let worker_state = DownloadWorkerState::new(2);
    let worker = DownloadWorker::new(pool.clone(), worker_state);

    let download_res = DownloadResult {
        file_path: flac_file.to_string_lossy().to_string(),
        bit_depth: 24,
        sample_rate: 96000,
        title: "Real HiRes Track".to_string(),
        artist: "Promotion Artist".to_string(),
        album: "Promotion Album".to_string(),
        service: "qobuz".to_string(),
        ..Default::default()
    };

    worker.mark_complete(queue_id, &download_res).await;

    // Verify tracks.audio_quality was promoted to 'hires' based on true disk reality
    let final_track_q: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(final_track_q.as_deref(), Some("hires"), "tracks.audio_quality must be promoted to 'hires'");

    // Verify downloads table
    let (bd, sr, fmt, q_dec, q_fb): (Option<i32>, Option<i32>, Option<String>, Option<String>, i64) = sqlx::query_as(
        "SELECT bit_depth, sample_rate, file_format, quality_decision, quality_fallback_used FROM downloads WHERE track_id = ?"
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(bd, Some(24));
    assert_eq!(sr, Some(96000));
    assert_eq!(fmt.as_deref(), Some("FLAC"));
    assert_eq!(q_dec.as_deref(), Some("CompletedExactQuality"));
    assert_eq!(q_fb, 0);
}
