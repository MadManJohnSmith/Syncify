//! S177 Grouping Tag Roundtrip Test Suite
//!
//! Validates:
//! 1. Derivation and writing of GROUPING Vorbis comment tag in FLAC containers.
//! 2. Roundtrip verification that GROUPING is properly preserved and readable.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
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

#[test]
fn test_flac_grouping_tag_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("grouping_track.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Planet Earth".to_string(),
        artist: "Duran Duran".to_string(),
        album: "Duran Duran".to_string(),
        album_artist: Some("Duran Duran".to_string()),
        grouping: Some("Duran Duran - Studio Albums".to_string()),
        track_number: 1,
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write");
    assert!(report.tags_match);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read flac tags");
    let comments = tag_obj.vorbis_comments().expect("vorbis comments");
    let grp_tags = comments.get("GROUPING").expect("GROUPING tag must exist");
    assert_eq!(grp_tags, &["Duran Duran - Studio Albums".to_string()]);
}
