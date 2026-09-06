//! TASK-109: Hi-Res Audio Quality Reconciliation and Post-Download Gate Tests
//!
//! Validates:
//! 1. A 16/44.1 audio file originally categorized as 'hires' is recategorized to 'lossless'.
//! 2. A 24/96 audio file legitimately retains the 'hires' category.
//! 3. The post-download quality gate rejects the 'hires' label for 16/44.1 streams and downgrades them to 'lossless'.
//! 4. Full SQLite database reconciliation correctly updates tracks, downloads, and decisions.

use sqlx::sqlite::SqlitePoolOptions;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use syncify_tauri_lib::download::audio_inspector::{
    classify_physical_audio_quality, enforce_post_download_quality_gate,
    inspect_physical_audio_file,
};
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::DownloadResult;
use syncify_tauri_lib::download::DownloadRequest;
use syncify_core_domain::quality::QualityDecisionKind;
use tempfile::TempDir;

/// Generates a valid minimal synthetic FLAC file with STREAMINFO metadata
fn create_synthetic_flac(path: &Path, sample_rate: u32, bit_depth: u8) {
    let mut data = Vec::new();
    // FLAC Magic
    data.extend_from_slice(b"fLaC");

    // METADATA_BLOCK_HEADER: last block = 1, type = 0 (STREAMINFO), length = 34 bytes
    data.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]);

    // min_block_size: 4096
    data.extend_from_slice(&[0x10, 0x00]);
    // max_block_size: 4096
    data.extend_from_slice(&[0x10, 0x00]);
    // min_frame_size: 14
    data.extend_from_slice(&[0x00, 0x00, 0x0E]);
    // max_frame_size: 14500
    data.extend_from_slice(&[0x00, 0x38, 0xA4]);

    // sample_rate (20 bits), channels (3 bits -> minus 1), bits_per_sample (5 bits -> minus 1)
    // total_samples (36 bits)
    let sr_20 = sample_rate & 0x000F_FFFF;
    let ch_minus_1 = 1u8; // 2 channels -> 1
    let bps_minus_1 = (bit_depth - 1) & 0x1F;

    let sample_first = ((sr_20 >> 4) & 0xFFFF) as u16;
    let sample_last_4 = ((sr_20 & 0x0F) as u8) << 4;
    let sample_channel_bps = sample_last_4 | (ch_minus_1 << 1) | ((bps_minus_1 >> 4) & 0x01);
    let next_byte = (bps_minus_1 & 0x0F) << 4; // total_samples hi 4 bits = 0

    data.push((sample_first >> 8) as u8);
    data.push((sample_first & 0xFF) as u8);
    data.push(sample_channel_bps);
    data.push(next_byte);

    // total_samples lo 32 bits (882000 samples)
    data.extend_from_slice(&[0x00, 0x0D, 0x75, 0x50]);

    // MD5 (16 zero bytes)
    data.extend_from_slice(&[0u8; 16]);

    let mut file = File::create(path).expect("Failed to create synthetic flac file");
    file.write_all(&data).expect("Failed to write flac data");
}

#[test]
fn test_classify_physical_audio_quality_matrix() {
    // 16-bit / 44.1kHz FLAC -> strictly lossless, never hires
    assert_eq!(classify_physical_audio_quality(16, 44100, "FLAC"), "lossless");
    // 16-bit / 48kHz FLAC -> strictly lossless, never hires
    assert_eq!(classify_physical_audio_quality(16, 48000, "FLAC"), "lossless");

    // 24-bit / 44.1kHz FLAC -> hires (bit_depth > 16)
    assert_eq!(classify_physical_audio_quality(24, 44100, "FLAC"), "hires");
    // 24-bit / 48kHz FLAC -> hires (bit_depth > 16)
    assert_eq!(classify_physical_audio_quality(24, 48000, "FLAC"), "hires");
    // 24-bit / 96kHz FLAC -> hires (bit_depth > 16 && sample_rate > 48000)
    assert_eq!(classify_physical_audio_quality(24, 96000, "FLAC"), "hires");
    // 24-bit / 192kHz FLAC -> hires
    assert_eq!(classify_physical_audio_quality(24, 192000, "FLAC"), "hires");
    // 16-bit / 96kHz FLAC -> hires (sample_rate > 48000)
    assert_eq!(classify_physical_audio_quality(16, 96000, "FLAC"), "hires");

    // Lossy codecs always classify as lossy regardless of sample rate
    assert_eq!(classify_physical_audio_quality(16, 44100, "AAC"), "lossy");
    assert_eq!(classify_physical_audio_quality(16, 44100, "MP3"), "lossy");
    assert_eq!(classify_physical_audio_quality(16, 48000, "OGG"), "lossy");
}

#[test]
fn test_enforce_post_download_quality_gate_behavior() {
    // Rejects 'hires' when physical reality is 16/44.1
    let tier_16_44 = enforce_post_download_quality_gate(Some("hires"), 16, 44100, "FLAC");
    assert_eq!(tier_16_44, "lossless");

    // Rejects 'hires' when physical reality is 16/48
    let tier_16_48 = enforce_post_download_quality_gate(Some("hires"), 16, 48000, "FLAC");
    assert_eq!(tier_16_48, "lossless");

    // Conserves 'hires' when physical reality is legitimately 24/96
    let tier_24_96 = enforce_post_download_quality_gate(Some("hires"), 24, 96000, "FLAC");
    assert_eq!(tier_24_96, "hires");

    // Conserves 'hires' when physical reality is 24/44.1
    let tier_24_44 = enforce_post_download_quality_gate(Some("hires"), 24, 44100, "FLAC");
    assert_eq!(tier_24_44, "hires");
}

#[test]
fn test_audio_inspector_synthetic_streaminfo_detection() {
    let temp_dir = TempDir::new().unwrap();
    let flac_16_path = temp_dir.path().join("track_16_44.flac");
    let flac_24_path = temp_dir.path().join("track_24_96.flac");

    create_synthetic_flac(&flac_16_path, 44100, 16);
    create_synthetic_flac(&flac_24_path, 96000, 24);

    let insp_16 = inspect_physical_audio_file(&flac_16_path).expect("Must inspect 16/44 FLAC");
    assert_eq!(insp_16.bit_depth, 16);
    assert_eq!(insp_16.sample_rate, 44100);
    assert_eq!(insp_16.canonical_quality(), "lossless");
    assert!(!insp_16.is_hires());

    let insp_24 = inspect_physical_audio_file(&flac_24_path).expect("Must inspect 24/96 FLAC");
    assert_eq!(insp_24.bit_depth, 24);
    assert_eq!(insp_24.sample_rate, 96000);
    assert_eq!(insp_24.canonical_quality(), "hires");
    assert!(insp_24.is_hires());
}

#[test]
fn test_orchestrator_reconciliation_and_post_download_gate() {
    let temp_dir = TempDir::new().unwrap();
    let flac_16_path = temp_dir.path().join("shortfall_16.flac");
    let flac_24_path = temp_dir.path().join("exact_24.flac");

    create_synthetic_flac(&flac_16_path, 44100, 16);
    create_synthetic_flac(&flac_24_path, 96000, 24);

    // 1. Test fake Hi-Res shortfall scenario
    let mut res_16 = DownloadResult {
        file_path: flac_16_path.to_str().unwrap().to_string(),
        bit_depth: 24, // Initially claimed by provider as 24-bit
        sample_rate: 96000, // Initially claimed by provider as 96kHz
        title: "False Hi-Res Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        release_date: None,
        track_number: 1,
        disc_number: 1,
        isrc: None,
        service: "qobuz".to_string(),
        origin_service: Some("qobuz".to_string()),
        origin_service_track_id: None,
        effective_service: Some("qobuz".to_string()),
        effective_service_track_id: None,
        fallback_reason: None,
        match_method: None,
        match_confidence: None,
        phase_timings: None,
        quality_decision: None,
        channels: Some(2),
        bitrate: None,
    };

    let req_hires = DownloadRequest {
        track_name: "False Hi-Res Track".to_string(),
        artist_name: "Test Artist".to_string(),
        album_name: "Test Album".to_string(),
        quality: "hires".to_string(),
        service_name: Some("qobuz".to_string()),
        strict_quality: false,
        allow_fallback: true,
        ..Default::default()
    };

    DownloadOrchestrator::reconcile_physical_audio_quality(&mut res_16, &req_hires);
    assert_eq!(res_16.bit_depth, 16);
    assert_eq!(res_16.sample_rate, 44100);
    assert!(res_16.quality_decision.is_some());
    let qd_16 = res_16.quality_decision.as_ref().unwrap();
    assert_eq!(qd_16.decision, QualityDecisionKind::CompletedWithQualityShortfall);
    assert!(qd_16.quality_fallback_used);

    let gate_tier_16 = DownloadOrchestrator::verify_post_download_quality_gate(&res_16);
    assert_eq!(gate_tier_16, "lossless");

    // 2. Test legitimate Hi-Res scenario
    let mut res_24 = DownloadResult {
        file_path: flac_24_path.to_str().unwrap().to_string(),
        bit_depth: 24,
        sample_rate: 96000,
        title: "Legitimate Hi-Res Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        release_date: None,
        track_number: 2,
        disc_number: 1,
        isrc: None,
        service: "qobuz".to_string(),
        origin_service: Some("qobuz".to_string()),
        origin_service_track_id: None,
        effective_service: Some("qobuz".to_string()),
        effective_service_track_id: None,
        fallback_reason: None,
        match_method: None,
        match_confidence: None,
        phase_timings: None,
        quality_decision: None,
        channels: Some(2),
        bitrate: None,
    };

    DownloadOrchestrator::reconcile_physical_audio_quality(&mut res_24, &req_hires);
    assert_eq!(res_24.bit_depth, 24);
    assert_eq!(res_24.sample_rate, 96000);
    assert!(res_24.quality_decision.is_some());
    let qd_24 = res_24.quality_decision.as_ref().unwrap();
    assert_eq!(qd_24.decision, QualityDecisionKind::CompletedExactQuality);
    assert!(!qd_24.quality_fallback_used);

    let gate_tier_24 = DownloadOrchestrator::verify_post_download_quality_gate(&res_24);
    assert_eq!(gate_tier_24, "hires");
}

#[tokio::test]
async fn test_sqlite_db_reconciliation_and_python_script_execution() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("syncify_test_task109.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test SQLite DB");

    // Apply all repository migrations
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must succeed");

    // Get service IDs
    let qobuz_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'qobuz'")
        .fetch_optional(&pool)
        .await
        .unwrap()
        .unwrap_or(1);

    // Create synthetic FLAC files
    let flac_16 = temp_dir.path().join("cd_quality.flac");
    let flac_24 = temp_dir.path().join("hires_quality.flac");
    create_synthetic_flac(&flac_16, 44100, 16);
    create_synthetic_flac(&flac_24, 96000, 24);

    // Track 101: Initially labeled as 'hires', but downloaded file is 16-bit / 44.1kHz
    sqlx::query("INSERT INTO tracks (id, title, audio_quality) VALUES (101, 'Fake Hi-Res Song', 'hires')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO downloads (
            track_id, source_service_id, file_path, file_format, bit_depth, sample_rate,
            requested_quality, quality_decision
        ) VALUES (101, ?, ?, 'FLAC', 16, 44100, 'hires', 'CompletedExactQuality')"#
    )
    .bind(qobuz_id)
    .bind(flac_16.to_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();

    // Track 102: Legitimate 24/96 Hi-Res track
    sqlx::query("INSERT INTO tracks (id, title, audio_quality) VALUES (102, 'True Hi-Res Song', 'hires')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO downloads (
            track_id, source_service_id, file_path, file_format, bit_depth, sample_rate,
            requested_quality, quality_decision
        ) VALUES (102, ?, ?, 'FLAC', 24, 96000, 'hires', 'CompletedExactQuality')"#
    )
    .bind(qobuz_id)
    .bind(flac_24.to_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();

    // Verify initial states
    let q101_pre: String = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 101")
        .fetch_one(&pool)
        .await
        .unwrap();
    let q102_pre: String = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 102")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q101_pre, "hires");
    assert_eq!(q102_pre, "hires");

    // Execute scripts/reconcile_hires_quality.py on this DB
    let py_status = Command::new("python3")
        .arg("../scripts/reconcile_hires_quality.py")
        .arg("--db-path")
        .arg(&db_path)
        .arg("--skip-backup")
        .status()
        .expect("Failed to execute python reconciliation script");
    assert!(py_status.success(), "Python maintenance script must exit 0");

    // Verify post-reconciliation states
    let q101_post: String = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 101")
        .fetch_one(&pool)
        .await
        .unwrap();
    let q102_post: String = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 102")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Track 101 (16/44.1) MUST be recategorized to 'lossless'
    assert_eq!(q101_post, "lossless", "16/44.1 track must be recategorized to lossless");

    // Track 102 (24/96) MUST retain 'hires'
    assert_eq!(q102_post, "hires", "24/96 track must legitimately retain hires");

    // Download 101 MUST be updated to CompletedWithQualityShortfall
    let (d101_decision, d101_fallback): (String, i64) = sqlx::query_as(
        "SELECT quality_decision, quality_fallback_used FROM downloads WHERE track_id = 101"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(d101_decision, "CompletedWithQualityShortfall");
    assert_eq!(d101_fallback, 1);

    // Download 102 MUST keep CompletedExactQuality
    let (d102_decision, d102_fallback): (String, i64) = sqlx::query_as(
        "SELECT quality_decision, quality_fallback_used FROM downloads WHERE track_id = 102"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(d102_decision, "CompletedExactQuality");
    assert_eq!(d102_fallback, 0);
}

#[tokio::test]
async fn test_worker_post_download_gate_downgrades_to_lossless() {
    use syncify_tauri_lib::worker::{DownloadWorker, DownloadWorkerState};

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("syncify_worker_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test SQLite DB");

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must succeed");

    // Track 201: Claimed Hi-Res in catalog
    sqlx::query("INSERT INTO tracks (id, title, audio_quality) VALUES (201, 'Claimed Hi-Res Song', 'hires')")
        .execute(&pool)
        .await
        .unwrap();

    let queue_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO download_queue (
            track_id, status, requested_quality, progress_percent
        ) VALUES (201, 'downloading', 'hires', 50.0) RETURNING id"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Physical file is only 16-bit / 44.1kHz
    let flac_file = temp_dir.path().join("clamped_stream.flac");
    create_synthetic_flac(&flac_file, 44100, 16);

    let worker_state = DownloadWorkerState::new(1);
    let worker = DownloadWorker::new(pool.clone(), worker_state);

    let download_res = DownloadResult {
        file_path: flac_file.to_string_lossy().to_string(),
        bit_depth: 16,
        sample_rate: 44100,
        title: "Claimed Hi-Res Song".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        service: "qobuz".to_string(),
        origin_service: Some("qobuz".to_string()),
        effective_service: Some("qobuz".to_string()),
        channels: Some(2),
        ..Default::default()
    };

    worker.mark_complete(queue_id, &download_res).await;

    // Verify tracks.audio_quality was updated to 'lossless'
    let track_quality: String = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 201")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(track_quality, "lossless", "Worker must update tracks.audio_quality to lossless when stream is 16/44.1");

    // Verify downloads ledger
    let (d_bd, d_sr, d_dec, d_fb): (i64, i64, String, i64) = sqlx::query_as(
        "SELECT bit_depth, sample_rate, quality_decision, quality_fallback_used FROM downloads WHERE track_id = 201"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(d_bd, 16);
    assert_eq!(d_sr, 44100);
    assert_eq!(d_dec, "CompletedWithQualityShortfall");
    assert_eq!(d_fb, 1);
}
