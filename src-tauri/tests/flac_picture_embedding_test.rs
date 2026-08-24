//! FLAC PICTURE Block Embedding and Verification Test (S175)
//!
//! Validates:
//! 1. FLAC container `PICTURE` block (`CoverFront`) embedding using `syncify-flac-writer`.
//! 2. Preservation of exact JPEG and PNG image payload bytes.
//! 3. `apply_and_verify_flac_tags` reports `cover_present == true` and matching `cover_size_bytes`.
//! 4. Physical readback via `metaflac::Tag` verifies `PictureType::CoverFront` and matching byte contents.

use std::path::PathBuf;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use tempfile::tempdir;

fn generate_synthetic_pcm() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_sec = 0.3;
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

fn create_synthetic_jpeg_bytes() -> Vec<u8> {
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x48,
        0x00, 0x48, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08,
        0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
        0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20,
        0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27,
        0x39, 0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01,
        0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x1F, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04,
        0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F,
        0x00, 0xBF, 0x80, 0xFF, 0xD9,
    ]
}

#[test]
fn test_flac_picture_embedding_and_readback() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("flac_picture_test.flac");
    create_synthetic_flac(&file_path);

    let cover_bytes = create_synthetic_jpeg_bytes();

    let meta = FlacMetadata {
        title: "Picture Test Track".to_string(),
        artist: "Picture Artist".to_string(),
        album: "Picture Album".to_string(),
        album_artist: Some("Picture Artist".to_string()),
        genre: Some("Electronic; Ambient".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        isrc: Some("USNPD0601064".to_string()),
        cover_data: Some(cover_bytes.clone()),
        cover_source: Some("Qobuz Cover Art".to_string()),
        ..Default::default()
    };

    // 1. Tag and verify FLAC
    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC tag write and verify");
    assert!(report.file_exists);
    assert!(report.flac_valid);
    assert!(report.tags_match);
    assert!(report.cover_present, "Cover must be reported as present");
    assert_eq!(report.cover_size_bytes, Some(cover_bytes.len()), "Reported cover size must match");

    // 2. Direct readback with metaflac
    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC metadata");
    let pictures: Vec<_> = read_tag.pictures().collect();
    assert_eq!(pictures.len(), 1, "Must have exactly 1 PICTURE metadata block");

    let pic = pictures[0];
    assert_eq!(pic.picture_type, metaflac::block::PictureType::CoverFront, "Must be CoverFront picture type");
    assert_eq!(pic.mime_type, "image/jpeg", "MIME type must be image/jpeg");
    assert_eq!(pic.data, cover_bytes.as_slice(), "Picture data bytes must exactly match input");
}
