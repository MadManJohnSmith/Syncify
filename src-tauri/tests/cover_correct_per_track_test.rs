//! Cover Correct Per Track Test (S176C)
//!
//! Validates:
//! 1. 0 missing covers: all processed tracks receive their embedded artwork.
//! 2. 0 collisions: tracks from different releases receive distinct artwork matching their respective releases.
//! 3. FLAC `PICTURE` (`CoverFront`) and M4A `covr` byte-level integrity.

use std::path::PathBuf;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
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

fn create_synthetic_m4a(path: &PathBuf) {
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
            "-c:a", "aac",
            "-b:a", "128k",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg must execute");

    assert!(status.status.success(), "ffmpeg AAC encoding must succeed");
    let _ = std::fs::remove_file(&temp_wav);
}

fn create_jpeg(seed: u8, len: usize) -> Vec<u8> {
    let mut v = vec![seed; len];
    v[0] = 0xFF;
    v[1] = 0xD8;
    v[len - 2] = 0xFF;
    v[len - 1] = 0xD9;
    v
}

#[test]
fn test_zero_collisions_and_zero_missing_covers_across_tracks() {
    let dir = tempdir().expect("tempdir");

    let cover_release_a = create_jpeg(0x11, 1024);
    let cover_release_b = create_jpeg(0x22, 2048);

    assert_ne!(cover_release_a, cover_release_b, "Cover payloads must be distinct");

    // Track 1 (Album A, Track 1 - FLAC)
    let t1_path = dir.path().join("01 - Track One.flac");
    create_synthetic_flac(&t1_path);
    let meta_t1 = FlacMetadata {
        title: "Track One".to_string(),
        artist: "Artist A".to_string(),
        album: "Album A".to_string(),
        track_number: 1,
        track_total: 2,
        disc_number: 1,
        disc_total: 1,
        cover_data: Some(cover_release_a.clone()),
        ..Default::default()
    };
    let rep_t1 = apply_and_verify_flac_tags(&t1_path, &meta_t1).expect("FLAC T1 write");
    assert!(rep_t1.cover_present, "T1 cover must not be missing");

    // Track 2 (Album A, Track 2 - FLAC)
    let t2_path = dir.path().join("02 - Track Two.flac");
    create_synthetic_flac(&t2_path);
    let meta_t2 = FlacMetadata {
        title: "Track Two".to_string(),
        artist: "Artist A".to_string(),
        album: "Album A".to_string(),
        track_number: 2,
        track_total: 2,
        disc_number: 1,
        disc_total: 1,
        cover_data: Some(cover_release_a.clone()),
        ..Default::default()
    };
    let rep_t2 = apply_and_verify_flac_tags(&t2_path, &meta_t2).expect("FLAC T2 write");
    assert!(rep_t2.cover_present, "T2 cover must not be missing");

    // Track 3 (Album B, Track 1 - M4A)
    let t3_path = dir.path().join("01 - Track Three.m4a");
    create_synthetic_m4a(&t3_path);
    let meta_t3 = Mp4Metadata {
        title: "Track Three".to_string(),
        artist: "Artist B".to_string(),
        album: "Album B".to_string(),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        cover_data: Some(cover_release_b.clone()),
        ..Default::default()
    };
    let rep_t3 = apply_and_verify_mp4_tags(&t3_path, &meta_t3).expect("M4A T3 write");
    assert!(rep_t3.cover_present, "T3 cover must not be missing");

    // Readback verification for T1
    let tag_t1 = metaflac::Tag::read_from_path(&t1_path).unwrap();
    let pics_t1: Vec<_> = tag_t1.pictures().collect();
    assert_eq!(pics_t1.len(), 1);
    assert_eq!(pics_t1[0].data, cover_release_a.as_slice());

    // Readback verification for T2
    let tag_t2 = metaflac::Tag::read_from_path(&t2_path).unwrap();
    let pics_t2: Vec<_> = tag_t2.pictures().collect();
    assert_eq!(pics_t2.len(), 1);
    assert_eq!(pics_t2[0].data, cover_release_a.as_slice());

    // Readback verification for T3
    let tag_t3 = mp4ameta::Tag::read_from_path(&t3_path).unwrap();
    let pic_t3 = tag_t3.artwork().or_else(|| tag_t3.artworks().next()).unwrap();
    assert_eq!(pic_t3.data, cover_release_b.as_slice());

    // Verify 0 collisions between Release A and Release B
    assert_ne!(pics_t1[0].data, pic_t3.data, "Track 1 and Track 3 must have distinct covers");
}
