//! S198 TASK-73: ReplayGain and EBU R128 Loudness Normalization Test Suite
//!
//! Validates:
//! 1. Migration 0083 applies cleanly and adds loudness & ReplayGain columns to `tracks`.
//! 2. `parse_ebur128_output` accurately computes integrated LUFS, True Peak, and gains.
//! 3. `calculate_album_replaygain` calculates multi-track album gain via energy summation.
//! 4. FLAC Vorbis comments roundtrip and verification via `apply_and_verify_flac_tags`.
//! 5. MP4 Apple SoundCheck `iTunNORM` calculation and formatting contract.
//! 6. SQLite persistence and index retrieval for loudness and ReplayGain fields.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::download::audio_inspector::{
    calculate_album_replaygain, parse_ebur128_output, LoudnessAnalysis,
};
use syncify_tauri_lib::services::mp4_writer::calculate_itunnorm;
use tempfile::tempdir;

fn generate_synthetic_pcm() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_sec = 0.1;
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        samples[i] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
    }
    samples
}

fn create_synthetic_flac(path: &PathBuf) {
    let samples = generate_synthetic_pcm();
    let temp_wav = path.with_extension("wav");

    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
    let sample_rate = 44100u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 2).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&block_align.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 2).to_le_bytes());

    for &s in &samples {
        let sample_i16 = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        wav_bytes.extend_from_slice(&sample_i16.to_le_bytes());
    }

    fs::write(&temp_wav, &wav_bytes).expect("Write wav");

    let status = Command::new("flac")
        .args([
            "-f",
            "--silent",
            "-o",
            path.to_str().unwrap(),
            temp_wav.to_str().unwrap(),
        ])
        .status()
        .expect("Run flac encoder");

    let _ = fs::remove_file(&temp_wav);
    assert!(status.success(), "flac encode must succeed");
}

#[tokio::test]
async fn test_migration_0083_application_and_schema() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Connect to in-memory SQLite");

    // Run all migrations including 0083
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // Verify tracks columns
    let pragma_rows = sqlx::query("PRAGMA table_info(tracks)")
        .fetch_all(&pool)
        .await
        .expect("Fetch table info");

    let column_names: Vec<String> = pragma_rows
        .iter()
        .map(|r| r.get::<String, _>("name"))
        .collect();

    assert!(column_names.contains(&"loudness".to_string()), "tracks must have 'loudness' column");
    assert!(
        column_names.contains(&"replaygain_track_gain".to_string()),
        "tracks must have 'replaygain_track_gain' column"
    );
    assert!(
        column_names.contains(&"replaygain_track_peak".to_string()),
        "tracks must have 'replaygain_track_peak' column"
    );
    assert!(
        column_names.contains(&"replaygain_album_gain".to_string()),
        "tracks must have 'replaygain_album_gain' column"
    );
    assert!(
        column_names.contains(&"replaygain_album_peak".to_string()),
        "tracks must have 'replaygain_album_peak' column"
    );

    // Verify index on loudness
    let index_rows = sqlx::query("PRAGMA index_list(tracks)")
        .fetch_all(&pool)
        .await
        .expect("Fetch index list");

    let index_names: Vec<String> = index_rows
        .iter()
        .map(|r| r.get::<String, _>("name"))
        .collect();

    assert!(
        index_names.contains(&"idx_tracks_loudness".to_string()),
        "tracks must have 'idx_tracks_loudness' index"
    );
}

#[test]
fn test_ebur128_parser_accuracy() {
    let sample_stderr = r#"
[Parsed_ebur128_0 @ 0x559979e29340] Summary:

  Integrated loudness:
    I:         -14.2 LUFS
    Threshold: -24.3 LUFS

  Loudness range:
    LRA:         6.5 LU
    Threshold: -34.4 LUFS
    LRA low:   -17.8 LUFS
    LRA high:  -11.3 LUFS

  True peak:
    Peak:       -0.5 dBFS
[out#0/null @ 0x559979e29340] video:0KiB audio:86KiB
"#;

    // Test with ReplayGain 2.0 reference (-18.0 LUFS)
    let analysis_rg = parse_ebur128_output(sample_stderr, -18.0).expect("Parse ebur128");
    assert_eq!(analysis_rg.integrated_lufs, -14.2);
    assert_eq!(analysis_rg.true_peak_dbtp, -0.5);
    assert_eq!(analysis_rg.loudness_range_lu, Some(6.5));
    // -18.0 - (-14.2) = -3.80 dB
    assert_eq!(analysis_rg.replaygain_track_gain, "-3.80 dB");
    assert_eq!(analysis_rg.r128_track_gain, "-8.80 LU");
    // Peak linear: 10^(-0.5/20) ~= 0.944061
    assert!(analysis_rg.track_peak > 0.94 && analysis_rg.track_peak < 0.95);

    // Test with streaming reference (-14.0 LUFS)
    let analysis_streaming = parse_ebur128_output(sample_stderr, -14.0).expect("Parse ebur128");
    // -14.0 - (-14.2) = +0.20 dB
    assert_eq!(analysis_streaming.replaygain_track_gain, "+0.20 dB");
}

#[test]
fn test_calculate_album_replaygain_energy_summation() {
    let t1 = LoudnessAnalysis {
        integrated_lufs: -14.0,
        true_peak_dbtp: -0.5,
        loudness_range_lu: Some(5.0),
        track_gain_db: -4.0,
        track_peak: 0.95,
        album_gain_db: None,
        album_peak: None,
        replaygain_track_gain: "-4.00 dB".to_string(),
        replaygain_track_peak: "0.950000".to_string(),
        replaygain_album_gain: None,
        replaygain_album_peak: None,
        r128_track_gain: "-9.00 LU".to_string(),
    };

    let t2 = LoudnessAnalysis {
        integrated_lufs: -16.0,
        true_peak_dbtp: -1.0,
        loudness_range_lu: Some(4.0),
        track_gain_db: -2.0,
        track_peak: 0.89,
        album_gain_db: None,
        album_peak: None,
        replaygain_track_gain: "-2.00 dB".to_string(),
        replaygain_track_peak: "0.890000".to_string(),
        replaygain_album_gain: None,
        replaygain_album_peak: None,
        r128_track_gain: "-7.00 LU".to_string(),
    };

    let album_res = calculate_album_replaygain(&[t1, t2], Some(-18.0))
        .expect("Album replaygain calculation");

    let (album_lufs, album_gain_db, album_gain_str, album_peak_str) = album_res;

    // Energy mean: (10^-1.4 + 10^-1.6) / 2 = (0.03981 + 0.02512) / 2 = 0.03246
    // Album LUFS = 10 * log10(0.03246) ~= -14.89 LUFS
    assert!((album_lufs - (-14.89)).abs() < 0.05);
    // Album gain = -18.0 - (-14.89) = -3.11 dB
    assert!((album_gain_db - (-3.11)).abs() < 0.05);
    assert_eq!(album_gain_str, "-3.11 dB");
    // Album peak is max(0.95, 0.89) = 0.950000
    assert_eq!(album_peak_str, "0.950000");
}

#[test]
fn test_flac_replaygain_vorbis_tags_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("replaygain_test_track.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Test Fluid Mix".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        loudness: Some(-13.5),
        replaygain_track_gain: Some("-4.50 dB".to_string()),
        replaygain_track_peak: Some("0.988220".to_string()),
        replaygain_album_gain: Some("-3.80 dB".to_string()),
        replaygain_album_peak: Some("0.999120".to_string()),
        replaygain_reference_loudness: Some("-18.0 LUFS".to_string()),
        r128_track_gain: Some("-9.50 LU".to_string()),
        ..Default::default()
    };

    let verification = apply_and_verify_flac_tags(&file_path, &meta)
        .expect("apply_and_verify_flac_tags must succeed");

    assert!(verification.tags_match, "Tags must match: {:?}", verification.mismatches);

    // Explicit readback using metaflac
    let tag = metaflac::Tag::read_from_path(&file_path).expect("Read metaflac tag");
    let vorbis = tag.vorbis_comments().expect("Vorbis comments");

    let get_val = |k: &str| -> Option<String> {
        vorbis.get(k).and_then(|vals| vals.first().cloned())
    };

    assert_eq!(get_val("REPLAYGAIN_TRACK_GAIN"), Some("-4.50 dB".to_string()));
    assert_eq!(get_val("REPLAYGAIN_TRACK_PEAK"), Some("0.988220".to_string()));
    assert_eq!(get_val("REPLAYGAIN_ALBUM_GAIN"), Some("-3.80 dB".to_string()));
    assert_eq!(get_val("REPLAYGAIN_ALBUM_PEAK"), Some("0.999120".to_string()));
    assert_eq!(get_val("REPLAYGAIN_REFERENCE_LOUDNESS"), Some("-18.0 LUFS".to_string()));
    assert_eq!(get_val("R128_TRACK_GAIN"), Some("-9.50 LU".to_string()));
    assert_eq!(get_val("LOUDNESS"), Some("-13.5".to_string()));
}

#[test]
fn test_mp4_itunnorm_calculation_and_format() {
    let itunnorm = calculate_itunnorm(-3.0, 0.988220, Some(-2.5), Some(0.995000));

    // Contract: starts with single leading space and 10 space-separated 8-digit hex values
    assert!(itunnorm.starts_with(' '), "iTunNORM must start with a leading space");
    let parts: Vec<&str> = itunnorm.trim().split_whitespace().collect();
    assert_eq!(parts.len(), 10, "iTunNORM must have 10 hex tokens");

    for p in &parts {
        assert_eq!(p.len(), 8, "Each token in iTunNORM must be 8 hex characters: {}", p);
        assert!(
            p.chars().all(|c| c.is_ascii_hexdigit()),
            "Token must be valid hexadecimal: {}",
            p
        );
    }
}

#[tokio::test]
async fn test_sqlite_persistence_and_query_loudness() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Connect to in-memory SQLite");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply");

    // Insert track with loudness metrics
    sqlx::query(
        r#"
        INSERT INTO tracks (
            title, loudness, replaygain_track_gain, replaygain_track_peak,
            replaygain_album_gain, replaygain_album_peak
        ) VALUES (?, ?, ?, ?, ?, ?)
        "#
    )
    .bind("Sweet Fade Track")
    .bind(-14.2)
    .bind("-3.80 dB")
    .bind("0.944061")
    .bind("-3.11 dB")
    .bind("0.950000")
    .execute(&pool)
    .await
    .expect("Insert track");

    // Query back via idx_tracks_loudness
    let row = sqlx::query(
        "SELECT title, loudness, replaygain_track_gain, replaygain_track_peak FROM tracks WHERE loudness < -10.0"
    )
    .fetch_one(&pool)
    .await
    .expect("Fetch track by loudness index");

    let title: String = row.get("title");
    let loudness: f64 = row.get("loudness");
    let track_gain: String = row.get("replaygain_track_gain");
    let track_peak: String = row.get("replaygain_track_peak");

    assert_eq!(title, "Sweet Fade Track");
    assert!((loudness - (-14.2)).abs() < 1e-6);
    assert_eq!(track_gain, "-3.80 dB");
    assert_eq!(track_peak, "0.944061");
}
