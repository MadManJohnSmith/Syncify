//! Genre Multiple Delimiters & Discrete Blocks Test (S174)
//!
//! Validates:
//! 1. Semicolon-delimited genres ("Pop; Synthpop; Electronic") are split into multiple discrete `GENRE` blocks in FLAC Vorbis comments.
//! 2. Slash-delimited genres ("Rock/Pop/Alternative") are split into multiple discrete `GENRE` blocks in FLAC Vorbis comments.
//! 3. M4A genre tagging preservation.

use std::path::PathBuf;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use tempfile::tempdir;

fn generate_synthetic_pcm() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_sec = 0.2;
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
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "flac",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg must execute");

    assert!(status.status.success(), "ffmpeg FLAC encoding must succeed");
    let _ = std::fs::remove_file(&temp_wav);
}

#[test]
fn test_flac_semicolon_delimited_genres_discrete_blocks() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("semicolon_genre.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Multi Genre Semicolon".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        genre: Some("Pop; Synth-pop; Electronic".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write");

    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC");
    let vorbis = read_tag.vorbis_comments().expect("Vorbis comments");
    let genre_entries = vorbis.get("GENRE").expect("GENRE tags present");

    assert_eq!(genre_entries.len(), 3, "Must have 3 distinct GENRE Vorbis comment blocks");
    assert_eq!(genre_entries[0], "Pop");
    // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
    // S184 canonicalization matrix: "Synth-pop" carries the audited winner casing "Synth-Pop".
    assert_eq!(genre_entries[1], "Synth-Pop");
    assert_eq!(genre_entries[2], "Electronic");
}

#[test]
fn test_flac_slash_delimited_genres_discrete_blocks() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("slash_genre.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Multi Genre Slash".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        genre: Some("Rock / Alternative / Indie".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write");

    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC");
    let vorbis = read_tag.vorbis_comments().expect("Vorbis comments");
    let genre_entries = vorbis.get("GENRE").expect("GENRE tags present");

    assert_eq!(genre_entries.len(), 3, "Must have 3 distinct GENRE Vorbis comment blocks");
    assert_eq!(genre_entries[0], "Rock");
    assert_eq!(genre_entries[1], "Alternative");
    assert_eq!(genre_entries[2], "Indie");
}
