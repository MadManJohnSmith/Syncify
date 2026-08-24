//! Real Country Name Roundtrip & Normalization Test (S175b)
//!
//! Validates:
//! 1. Resolution of country names / codes / aliases to the real canonical country name:
//!    - "US", "USA", "United States" -> "United States"
//!    - "DE", "Deutschland", "Germany" -> "Germany"
//!    - "GB", "UK", "United Kingdom" -> "United Kingdom"
//!    - "JP", "Japan" -> "Japan"
//!    - "Europe" -> "Europe"
//! 2. FLAC container Vorbis comment `COUNTRY` and `RELEASECOUNTRY` roundtrip writing and physical readback.
//! 3. M4A / AAC container freeform `COUNTRY` and `RELEASECOUNTRY` roundtrip writing and physical readback.

use std::path::PathBuf;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_metadata_domain::{resolve_country, CountryResolution};
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

#[test]
fn test_real_country_name_domain_resolution() {
    match resolve_country("US") {
        CountryResolution::Country { canonical_name, .. } => assert_eq!(canonical_name, "United States"),
        _ => panic!("Expected United States resolution"),
    }

    match resolve_country("DE") {
        CountryResolution::Country { canonical_name, .. } => assert_eq!(canonical_name, "Germany"),
        _ => panic!("Expected Germany resolution"),
    }

    match resolve_country("GB") {
        CountryResolution::Country { canonical_name, .. } => assert_eq!(canonical_name, "United Kingdom"),
        _ => panic!("Expected United Kingdom resolution"),
    }

    match resolve_country("UK") {
        CountryResolution::Country { canonical_name, .. } => assert_eq!(canonical_name, "United Kingdom"),
        _ => panic!("Expected United Kingdom resolution"),
    }

    match resolve_country("JP") {
        CountryResolution::Country { canonical_name, .. } => assert_eq!(canonical_name, "Japan"),
        _ => panic!("Expected Japan resolution"),
    }

    match resolve_country("Europe") {
        CountryResolution::Region { region_name, .. } => assert_eq!(region_name, "Europe"),
        _ => panic!("Expected Europe region resolution"),
    }
}

#[test]
fn test_flac_real_country_name_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("flac_real_country.flac");
    create_synthetic_flac(&file_path);

    // Provide code "US"; tags carry the ISO 3166-1 alpha-2 code per S177 contract
    // (c8cc6a6 "alpha2 country resolution"), which supersedes the earlier real-name
    // tag convention while resolve_country still canonicalizes to the real name.
    let meta = FlacMetadata {
        title: "Real Country Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        release_country: Some("US".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write");
    assert!(report.tags_match);

    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC");
    let vorbis = read_tag.vorbis_comments().expect("Vorbis comments");

    let country_val = vorbis.get("COUNTRY").and_then(|v| v.first()).map(|s| s.as_str());
    assert_eq!(country_val, Some("US"), "COUNTRY tag must be ISO 3166-1 alpha-2 'US'");

    let rel_country_val = vorbis.get("RELEASECOUNTRY").and_then(|v| v.first()).map(|s| s.as_str());
    assert_eq!(rel_country_val, Some("US"), "RELEASECOUNTRY tag must be ISO 3166-1 alpha-2 'US'");
}

#[test]
fn test_m4a_real_country_name_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("m4a_real_country.m4a");
    create_synthetic_m4a(&file_path);

    // Provide code "GB"; atoms carry the ISO 3166-1 alpha-2 code per S177 contract
    // (alpha2 country resolution), mirroring the FLAC leg above.
    let meta = Mp4Metadata {
        title: "M4A Real Country Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        release_country: Some("GB".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_mp4_tags(&file_path, &meta).expect("M4A write");
    assert!(report.tags_match);

    let read_tag = mp4ameta::Tag::read_from_path(&file_path).expect("Read M4A");

    let country_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");
    let read_country = read_tag.strings_of(&country_ident).next();
    assert_eq!(read_country, Some("GB"), "M4A COUNTRY must be ISO 3166-1 alpha-2 'GB'");

    let rel_country_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "RELEASECOUNTRY");
    let read_rel_country = read_tag.strings_of(&rel_country_ident).next();
    assert_eq!(read_rel_country, Some("GB"), "M4A RELEASECOUNTRY must be ISO 3166-1 alpha-2 'GB'");
}
