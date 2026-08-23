//! BPM Container Roundtrip & Physical Invariance Test (S173)
//!
//! Validates:
//! 1. M4A AAC container `tmpo` atom tagging, physical readback, and audio payload hash invariance.
//! 2. Hi-Res FLAC (24-bit / 96kHz) Vorbis comment tagging (`BPM`, `TEMPO`), physical readback, and audio payload hash invariance.
//! 3. Corrupt/truncated audio file safe rejection without panic or data corruption.

use std::path::PathBuf;
use syncify_tauri_lib::services::repair_guardrail::compute_file_audio_content_hash;
use syncify_tauri_lib::services::tempo_analyzer::TempoAnalyzer;
use tempfile::tempdir;

fn generate_sine_pcm(freq: f32, sample_rate: u32, duration_sec: f64) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        samples[i] = (2.0 * std::f32::consts::PI * freq * t).sin() * 0.7;
    }
    samples
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

fn create_hires_flac_from_pcm(path: &PathBuf, samples: &[f32], sample_rate: u32) {
    let temp_wav = path.with_extension("wav");

    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 3; // 24-bit
    let block_align = 3u16;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 3).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&block_align.to_le_bytes());
    wav_bytes.extend_from_slice(&24u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 3).to_le_bytes());

    for &s in samples {
        let i32_sample = (s.clamp(-1.0, 1.0) * 8388607.0) as i32;
        let b = i32_sample.to_le_bytes();
        wav_bytes.push(b[0]);
        wav_bytes.push(b[1]);
        wav_bytes.push(b[2]);
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "flac",
            "-sample_fmt", "s32",
            "-ar", &sample_rate.to_string(),
            path.to_str().unwrap(),
        ])
        .output();

    let _ = std::fs::remove_file(&temp_wav);
}

#[tokio::test]
async fn test_m4a_bpm_tag_roundtrip_test() {
    let dir = tempdir().unwrap();
    let m4a_path = dir.path().join("roundtrip_test.m4a");

    let samples = generate_sine_pcm(440.0, 44100, 3.0);
    create_m4a_from_pcm(&m4a_path, &samples, 44100);

    if !m4a_path.exists() {
        eprintln!("ffmpeg not available to create M4A fixture");
        return;
    }

    let hash_before = compute_file_audio_content_hash(&m4a_path)
        .await
        .expect("Compute pre-tagging audio payload hash");

    // Tag M4A with BPM = 124
    TempoAnalyzer::retag_file_with_bpm(&m4a_path, 124)
        .await
        .expect("Retagging M4A with BPM 124 must succeed");

    // Physical re-read validation with mp4ameta
    let tag = mp4ameta::Tag::read_from_path(&m4a_path).expect("Read M4A tag");
    assert_eq!(tag.bpm(), Some(124), "M4A tmpo atom must match 124");

    let hash_after = compute_file_audio_content_hash(&m4a_path)
        .await
        .expect("Compute post-tagging audio payload hash");

    assert_eq!(
        hash_before, hash_after,
        "Audio payload hash must remain 100% invariant after M4A tagging"
    );
}

#[tokio::test]
async fn test_hires_flac_bpm_tag_roundtrip_test() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("hires_96k_24bit.flac");

    // 96 kHz Hi-Res audio
    let samples = generate_sine_pcm(880.0, 96000, 3.0);
    create_hires_flac_from_pcm(&flac_path, &samples, 96000);

    if !flac_path.exists() {
        eprintln!("ffmpeg not available to create Hi-Res FLAC fixture");
        return;
    }

    let hash_before = compute_file_audio_content_hash(&flac_path)
        .await
        .expect("Compute pre-tagging Hi-Res FLAC audio payload hash");

    // Tag Hi-Res FLAC with BPM = 132
    TempoAnalyzer::retag_file_with_bpm(&flac_path, 132)
        .await
        .expect("Retagging Hi-Res FLAC with BPM 132 must succeed");

    // Physical re-read validation with metaflac
    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Read FLAC tag");
    let vc = tag.vorbis_comments().expect("Vorbis comments present");
    assert_eq!(vc.get("BPM").unwrap()[0], "132");
    assert_eq!(vc.get("TEMPO").unwrap()[0], "132");

    let hash_after = compute_file_audio_content_hash(&flac_path)
        .await
        .expect("Compute post-tagging Hi-Res FLAC audio payload hash");

    assert_eq!(
        hash_before, hash_after,
        "Hi-Res 24-bit 96kHz audio payload hash must remain 100% invariant after FLAC tagging"
    );
}

#[tokio::test]
async fn test_corrupt_audio_test() {
    let dir = tempdir().unwrap();
    let corrupt_flac = dir.path().join("corrupted.flac");

    // Write random garbage bytes
    std::fs::write(&corrupt_flac, b"fLaC\x00\x00\x00\x22GARBAGE_DATA_TRUNCATED_HEADER")
        .unwrap();

    let res = TempoAnalyzer::analyze_file(&corrupt_flac, 0.40).await;
    assert!(
        res.is_err(),
        "Corrupted audio file must return Err safely without panicking"
    );
}
