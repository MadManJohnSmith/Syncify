//! S176M Exhaustive Enrichment - Country Resolution & Tag Writing Test Suite
//!
//! Tests:
//! 1. Resolution of ISO country codes and localized names to real canonical country names (United States, Germany, United Kingdom, Japan, France, etc.).
//! 2. Multi-provider conflict resolution prioritizing official streaming release provider (Qobuz/Tidal) or MusicBrainz over Spotify/Inferred.
//! 3. `resolve_exhaustive_track_metadata` populating real country name into `release_country`.
//! 4. Writing ISO 3166-1 alpha-2 codes to FLAC VorbisComments (RELEASECOUNTRY and COUNTRY) per S177 contract (c8cc6a6).
//! 5. Writing ISO 3166-1 alpha-2 codes to MP4 / M4A freeform atoms (----:com.apple.iTunes:COUNTRY and RELEASECOUNTRY) per S177 contract (c8cc6a6).

use std::fs::File;
use std::io::Write;
use std::path::Path;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_metadata_domain::{fuse_countries, resolve_country, CountryResolution};
use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
use tempfile::TempDir;

fn create_minimal_flac(path: &Path) {
    let mut file = File::create(path).unwrap();
    file.write_all(b"fLaC").unwrap();
    let streaminfo_header = [0x00, 0x00, 0x00, 0x22];
    file.write_all(&streaminfo_header).unwrap();
    let streaminfo_data = [
        0x10, 0x00, 0x10, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x0A, 0xC4, 0x42, 0xF0, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    file.write_all(&streaminfo_data).unwrap();
    let padding_header = [0x81, 0x00, 0x00, 0x10];
    file.write_all(&padding_header).unwrap();
    file.write_all(&[0x00; 16]).unwrap();
}

fn create_minimal_mp4(path: &Path) {
    let sample_rate: u32 = 44100;
    let duration_sec = 0.2;
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut wav_bytes = Vec::new();
    let num_samples = total_samples as u32;
    let byte_rate: u32 = sample_rate * 2;
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

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    let temp_wav = path.with_extension("wav");
    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            temp_wav.to_str().unwrap(),
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg must execute");

    assert!(status.status.success(), "ffmpeg AAC encoding must succeed");
    let _ = std::fs::remove_file(&temp_wav);
}

#[test]
fn test_country_canonical_name_resolution() {
    let check = |input: &str, expected: &str| {
        match resolve_country(input) {
            CountryResolution::Country { canonical_name, .. } => assert_eq!(canonical_name, expected),
            other => panic!("Expected Country resolution for '{}', got {:?}", input, other),
        }
    };

    check("US", "United States");
    check("USA", "United States");
    check("United States of America", "United States");

    check("GB", "United Kingdom");
    check("UK", "United Kingdom");
    check("Great Britain", "United Kingdom");

    check("DE", "Germany");
    check("DEU", "Germany");
    check("Deutschland", "Germany");

    check("JP", "Japan");
    check("JPN", "Japan");
    check("Japon", "Japan");

    check("FR", "France");
    check("FRA", "France");
}

#[test]
fn test_country_fusion_precedence() {
    // 1. StreamingService (Qobuz: GB) beats Spotify (US)
    let candidates = [
        ("US", "spotify", 0.80),
        ("GB", "qobuz", 0.90),
    ];
    let fused = fuse_countries(&candidates);
    assert_eq!(fused, Some("United Kingdom".to_string()));

    // 2. MusicBrainz beats Spotify
    let mb_candidates = [
        ("US", "spotify", 0.80),
        ("Germany", "musicbrainz", 0.85),
    ];
    let fused_mb = fuse_countries(&mb_candidates);
    assert_eq!(fused_mb, Some("Germany".to_string()));
}

#[tokio::test]
async fn test_exhaustive_enrichment_country_multi_provider_resolution() {
    let engine = EnrichmentEngine::new();

    let qobuz_source = OriginTrackMetadata {
        title: Some("Radioactivity".to_string()),
        artist: Some("Kraftwerk".to_string()),
        album: Some("Radio-Activity".to_string()),
        release_country: Some("DE".to_string()),
        source_name: "qobuz".to_string(),
        ..Default::default()
    };

    let spotify_source = OriginTrackMetadata {
        title: Some("Radioactivity".to_string()),
        artist: Some("Kraftwerk".to_string()),
        album: Some("Radio-Activity".to_string()),
        release_country: Some("US".to_string()),
        source_name: "spotify".to_string(),
        ..Default::default()
    };

    let enriched = engine.resolve_exhaustive_track_metadata(
        "Kraftwerk",
        "Radio-Activity",
        "Radioactivity",
        None,
        &[qobuz_source, spotify_source],
        false,
    ).await;

    assert_eq!(enriched.release_country.value(), Some("Germany"));
}

#[test]
fn test_flac_country_tag_writing_real_name_and_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("country_test.flac");
    create_minimal_flac(&flac_path);

    let meta = FlacMetadata {
        title: "Autobahn".to_string(),
        artist: "Kraftwerk".to_string(),
        album: "Autobahn".to_string(),
        release_country: Some("DE".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&flac_path, &meta).unwrap();

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();

    // S177 contract (c8cc6a6): tags carry the ISO 3166-1 alpha-2 code for sovereign countries.
    assert_eq!(comments.get("RELEASECOUNTRY").unwrap(), &["DE"]);
    assert_eq!(comments.get("COUNTRY").unwrap(), &["DE"]);
}

#[test]
fn test_mp4_country_tag_writing_real_name_and_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let mp4_path = temp_dir.path().join("country_test.m4a");
    create_minimal_mp4(&mp4_path);

    let meta = Mp4Metadata {
        title: "Autobahn".to_string(),
        artist: "Kraftwerk".to_string(),
        album: "Autobahn".to_string(),
        release_country: Some("Germany".to_string()),
        ..Default::default()
    };

    apply_and_verify_mp4_tags(&mp4_path, &meta).unwrap();

    let tag = mp4ameta::Tag::read_from_path(&mp4_path).unwrap();
    let ident_rc = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "RELEASECOUNTRY");
    let ident_c = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");

    // S177 contract (c8cc6a6): freeform atoms carry the ISO 3166-1 alpha-2 code.
    assert_eq!(tag.strings_of(&ident_rc).next().unwrap(), "DE");
    assert_eq!(tag.strings_of(&ident_c).next().unwrap(), "DE");
}
