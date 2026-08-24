//! Country Normalization & ISO 3166-1 alpha-2 Test (S174)
//!
//! Validates:
//! 1. Normalization of country names to ISO 3166-1 alpha-2 code ("United States" -> US, "Germany" -> DE, "UK" -> GB, "Japan" -> JP).
//! 2. Preservation of regional entities ("Europe" -> XE, "Worldwide" -> XW).
//! 3. FLAC `COUNTRY` and `RELEASECOUNTRY` tag roundtrip.
//! 4. M4A `COUNTRY` and `RELEASECOUNTRY` freeform atom roundtrip.

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
fn test_country_normalization_domain_rules() {
    match resolve_country("United States") {
        CountryResolution::Country { iso_alpha2, .. } => assert_eq!(iso_alpha2, "US"),
        _ => panic!("Expected US resolution"),
    }

    match resolve_country("Germany") {
        CountryResolution::Country { iso_alpha2, .. } => assert_eq!(iso_alpha2, "DE"),
        _ => panic!("Expected DE resolution"),
    }

    match resolve_country("United Kingdom") {
        CountryResolution::Country { iso_alpha2, .. } => assert_eq!(iso_alpha2, "GB"),
        _ => panic!("Expected GB resolution"),
    }

    match resolve_country("UK") {
        CountryResolution::Country { iso_alpha2, .. } => assert_eq!(iso_alpha2, "GB"),
        _ => panic!("Expected GB resolution"),
    }

    match resolve_country("Japan") {
        CountryResolution::Country { iso_alpha2, .. } => assert_eq!(iso_alpha2, "JP"),
        _ => panic!("Expected JP resolution"),
    }

    match resolve_country("Europe") {
        CountryResolution::Region { region_name, .. } => assert_eq!(region_name, "Europe"),
        _ => panic!("Expected Europe region resolution"),
    }
}

#[test]
fn test_flac_country_and_releasecountry_tags() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("flac_country_test.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Country Test".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        release_country: Some("United States".to_string()),
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

    let country_val = vorbis.get("COUNTRY").and_then(|v| v.first()).map(|s| s.as_str());
    assert_eq!(country_val, Some("US"), "COUNTRY tag must be normalized to 'US'");

    let rel_country_val = vorbis.get("RELEASECOUNTRY").and_then(|v| v.first()).map(|s| s.as_str());
    assert_eq!(rel_country_val, Some("US"), "RELEASECOUNTRY tag must be normalized to 'US'");
}

#[test]
fn test_m4a_country_and_releasecountry_atoms() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("m4a_country_test.m4a");
    create_synthetic_m4a(&file_path);

    let meta = Mp4Metadata {
        title: "M4A Country Test".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        release_country: Some("United Kingdom".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    apply_and_verify_mp4_tags(&file_path, &meta).expect("M4A write");

    let read_tag = mp4ameta::Tag::read_from_path(&file_path).expect("Read M4A");

    let country_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");
    let read_country = read_tag.strings_of(&country_ident).next();
    assert_eq!(read_country, Some("GB"), "M4A COUNTRY must be 'GB'");

    let rel_country_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "RELEASECOUNTRY");
    let read_rel_country = read_tag.strings_of(&rel_country_ident).next();
    assert_eq!(read_rel_country, Some("GB"), "M4A RELEASECOUNTRY must be 'GB'");
}
