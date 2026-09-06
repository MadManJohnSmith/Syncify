//! Rhythm & Key Metadata (BPM, INITIALKEY, Energy, Camelot) Test Suite (TASK-74)
//!
//! Validates:
//! 1. Emission and physical verification of FLAC tags: `BPM`, `TEMPO`, `TBPM`, `INITIALKEY`, `KEY`.
//! 2. Emission and physical verification of MP4 tags: `tmpo`, `©tmp`, `INITIALKEY`, `KEY`.
//! 3. DSP extraction of BPM, harmonic key in standard Camelot notation (1A-12B), and energy.
//! 4. Atomic database persistence of `tracks.bpm`, `tracks.musical_key`, `tracks.energy`.
//! 5. Audio payload SHA-256 invariance guard on rhythm/key retagging.

use std::path::{Path, PathBuf};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
use syncify_tauri_lib::services::repair_guardrail::compute_file_audio_content_hash;
use syncify_tauri_lib::services::tempo_analyzer::{
    normalize_to_camelot, root_and_mode_to_camelot, TempoAnalyzer,
};
use tempfile::tempdir;

fn generate_melodic_rhythmic_audio(
    bpm: f64,
    root_freq: f32,
    sample_rate: u32,
    duration_sec: f64,
) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    let beat_interval = (sample_rate as f64 * 60.0 / bpm) as usize;

    // Synthesize kick/snare beat on beat_interval
    for beat_start in (0..total_samples).step_by(beat_interval) {
        let pulse_len = (sample_rate as usize / 10).min(total_samples - beat_start);
        for i in 0..pulse_len {
            let t = i as f32 / sample_rate as f32;
            let decay = (-35.0 * t).exp();
            let freq = 120.0 - (60.0 * t);
            let sine = (2.0 * std::f32::consts::PI * freq * t).sin();
            samples[beat_start + i] += sine * decay * 0.7;
        }
    }

    // Add continuous harmonic tone / chord (root, minor third, fifth)
    let f_root = root_freq;
    let f_third = root_freq * 1.189207; // Minor third
    let f_fifth = root_freq * 1.498307; // Perfect fifth

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let s_root = (2.0 * std::f32::consts::PI * f_root * t).sin() * 0.25;
        let s_third = (2.0 * std::f32::consts::PI * f_third * t).sin() * 0.15;
        let s_fifth = (2.0 * std::f32::consts::PI * f_fifth * t).sin() * 0.15;
        samples[i] += s_root + s_third + s_fifth;
    }

    samples
}

fn create_flac_from_pcm(path: &Path, samples: &[f32], sample_rate: u32) {
    let temp_wav = path.with_extension("wav");
    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
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

    for &s in samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "flac",
            path.to_str().unwrap(),
        ])
        .output();

    let _ = std::fs::remove_file(&temp_wav);
}

fn create_m4a_from_pcm(path: &PathBuf, samples: &[f32], sample_rate: u32) {
    let temp_wav = path.with_extension("wav");
    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
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

    for &s in samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "aac",
            "-b:a", "256k",
            path.to_str().unwrap(),
        ])
        .output();

    let _ = std::fs::remove_file(&temp_wav);
}

#[tokio::test]
async fn test_flac_rhythm_and_key_tag_emission_and_verification() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("flac_rhythm_test.flac");
    let samples = generate_melodic_rhythmic_audio(124.0, 440.0, 22050, 2.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);

    let meta = FlacMetadata {
        title: "Harmonic Mixing Track".to_string(),
        artist: "Test Producer".to_string(),
        album: "Electronic Anthology".to_string(),
        bpm: Some(124),
        initial_key: Some("8A".to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&flac_path, &meta)
        .expect("apply_and_verify_flac_tags must succeed");

    assert!(report.tags_match, "Tags must match without mismatches: {:?}", report.mismatches);
    assert!(report.bpm_present, "BPM must be flagged as present");

    // Physical readback via metaflac
    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Read FLAC tags");
    let vc = tag.vorbis_comments().expect("Vorbis comments present");

    assert_eq!(vc.get("BPM").and_then(|v| v.first()), Some(&"124".to_string()));
    assert_eq!(vc.get("TEMPO").and_then(|v| v.first()), Some(&"124".to_string()));
    assert_eq!(vc.get("TBPM").and_then(|v| v.first()), Some(&"124".to_string()));
    assert_eq!(vc.get("INITIALKEY").and_then(|v| v.first()), Some(&"8A".to_string()));
    assert_eq!(vc.get("KEY").and_then(|v| v.first()), Some(&"8A".to_string()));
}

#[tokio::test]
async fn test_mp4_rhythm_and_key_tag_emission_and_verification() {
    let dir = tempdir().unwrap();
    let m4a_path = dir.path().join("m4a_rhythm_test.m4a");
    let samples = generate_melodic_rhythmic_audio(128.0, 440.0, 22050, 2.0);
    create_m4a_from_pcm(&m4a_path, &samples, 22050);

    let meta = Mp4Metadata {
        title: "Tidal Radio Stream".to_string(),
        artist: "Harmonic DJ".to_string(),
        album: "Club Essentials".to_string(),
        bpm: Some(128),
        initial_key: Some("11B".to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_mp4_tags(&m4a_path, &meta)
        .expect("apply_and_verify_mp4_tags must succeed");

    assert!(report.tags_match, "MP4 tags must match: {:?}", report.mismatches);

    // Physical readback via mp4ameta
    let tag = mp4ameta::Tag::read_from_path(&m4a_path).expect("Read M4A tag");
    assert_eq!(tag.bpm(), Some(128), "tmpo atom must be 128");

    let tmp_str = tag.strings_of(&mp4ameta::Fourcc(*b"\xa9tmp")).next();
    assert_eq!(tmp_str, Some("128"), "©tmp atom must be '128'");

    let key_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "INITIALKEY");
    let key_val = tag.strings_of(&key_ident).next();
    assert_eq!(key_val, Some("11B"), "INITIALKEY atom must be '11B'");
}

#[tokio::test]
async fn test_camelot_wheel_normalization_and_mapping() {
    // Standard Camelot mappings
    assert_eq!(normalize_to_camelot("8A"), Some("8A".to_string()));
    assert_eq!(normalize_to_camelot("11B"), Some("11B".to_string()));
    assert_eq!(normalize_to_camelot("01A"), Some("1A".to_string()));
    assert_eq!(normalize_to_camelot("12b"), Some("12B".to_string()));

    // Musical key names
    assert_eq!(normalize_to_camelot("Am"), Some("8A".to_string()));
    assert_eq!(normalize_to_camelot("A minor"), Some("8A".to_string()));
    assert_eq!(normalize_to_camelot("C"), Some("8B".to_string()));
    assert_eq!(normalize_to_camelot("C major"), Some("8B".to_string()));
    assert_eq!(normalize_to_camelot("C#m"), Some("12A".to_string()));
    assert_eq!(normalize_to_camelot("Dbm"), Some("12A".to_string()));
    assert_eq!(normalize_to_camelot("F# minor"), Some("11A".to_string()));
    assert_eq!(normalize_to_camelot("A"), Some("11B".to_string()));
    assert_eq!(normalize_to_camelot("G#m"), Some("1A".to_string()));

    // Wheel generator verification
    assert_eq!(root_and_mode_to_camelot(0, true), "8B");  // C Major
    assert_eq!(root_and_mode_to_camelot(9, false), "8A"); // A Minor
    assert_eq!(root_and_mode_to_camelot(9, true), "11B"); // A Major
    assert_eq!(root_and_mode_to_camelot(8, false), "1A"); // G# Minor
}

#[tokio::test]
async fn test_dsp_acoustic_feature_analysis_camelot_and_energy() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("dsp_analysis_test.flac");
    // Generate rhythmic audio at 120 BPM with A4 (440Hz) minor chord
    let samples = generate_melodic_rhythmic_audio(120.0, 440.0, 22050, 5.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);

    let result = TempoAnalyzer::analyze_acoustic_file(&flac_path, 0.35)
        .await
        .expect("Acoustic analysis must succeed");

    assert!(result.bpm.is_some(), "BPM must be detected");
    let bpm = result.bpm.unwrap();
    assert!(
        (bpm as i32 - 120).abs() <= 3,
        "Detected BPM {} should be near 120",
        bpm
    );

    assert!(result.key.is_some(), "Harmonic key must be detected");
    let key = result.key.unwrap();
    assert!(
        key == "8A" || key == "11B" || key.ends_with('A') || key.ends_with('B'),
        "Key '{}' should be valid Camelot notation",
        key
    );

    assert!(result.energy.is_some(), "Energy must be estimated");
    let energy = result.energy.unwrap();
    assert!(
        energy >= 0.05 && energy <= 1.0,
        "Energy {} should be in [0.05, 1.0]",
        energy
    );
}

#[tokio::test]
async fn test_rhythm_and_key_database_persistence() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Insert track without BPM, musical_key, or energy
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Harmonic Radio Track', 'GBAYE2400010') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("db_persistence_track.flac");
    let samples = generate_melodic_rhythmic_audio(126.0, 440.0, 22050, 4.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'FLAC')"
    )
    .bind(track_id)
    .bind(flac_path.to_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();

    // 2. Run analysis and retag
    let res = TempoAnalyzer::analyze_and_retag_track(&pool, track_id, 0.35, true)
        .await
        .expect("analyze_and_retag_track must succeed");

    assert!(res.bpm.is_some(), "Track must have detected BPM");

    // 3. Verify SQLite persistence
    let (bpm_val, key_val, energy_val, source_val): (Option<f64>, Option<String>, Option<f64>, Option<String>) =
        sqlx::query_as("SELECT bpm, musical_key, energy, tempo_source FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert!(bpm_val.is_some(), "tracks.bpm must be persisted in SQLite");
    assert!(key_val.is_some(), "tracks.musical_key must be persisted in SQLite");
    assert!(energy_val.is_some(), "tracks.energy must be persisted in SQLite");
    assert_eq!(source_val, Some("LocalAudioAnalysis".to_string()));

    let key = key_val.unwrap();
    assert!(
        key.ends_with('A') || key.ends_with('B'),
        "persisted key '{}' must be Camelot notation",
        key
    );

    // 4. Verify physical tags on disk
    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Read retagged FLAC");
    let vc = tag.vorbis_comments().expect("Vorbis comments present");
    assert!(vc.get("BPM").is_some(), "Physical FLAC tag BPM must be present");
    assert!(vc.get("INITIALKEY").is_some(), "Physical FLAC tag INITIALKEY must be present");
}

#[tokio::test]
async fn test_audio_payload_sha256_invariance_on_retagging() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("invariance_track.flac");
    let samples = generate_melodic_rhythmic_audio(120.0, 440.0, 22050, 2.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);

    let hash_before = compute_file_audio_content_hash(&flac_path)
        .await
        .expect("Compute initial hash");

    TempoAnalyzer::retag_file_with_rhythm_and_key(&flac_path, Some(132), Some("8A"))
        .await
        .expect("retag_file_with_rhythm_and_key must succeed");

    let hash_after = compute_file_audio_content_hash(&flac_path)
        .await
        .expect("Compute hash after tagging");

    assert_eq!(
        hash_before, hash_after,
        "Audio payload SHA-256 hash must be 100% identical after retagging"
    );
}
