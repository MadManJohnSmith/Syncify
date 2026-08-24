//! S176M Exhaustive Enrichment - Genre Fusion & Multi-Block Tag Writing Test Suite
//!
//! Tests:
//! 1. Multi-provider genre collection across Qobuz, Tidal, Spotify, and MusicBrainz.
//! 2. Splitting by ';' and '/', casing normalization, and case-insensitive deduplication.
//! 3. Preservation of non-English genres (e.g. French, Spanish) without language filtering.
//! 4. Writing multiple GENRE VorbisComment blocks in FLAC (`comments.get("GENRE")` has multiple entries).
//! 5. Writing and verifying fused genres in MP4 / M4A (`©gen` and `----:com.apple.iTunes:GENRE`).

use std::fs::File;
use std::io::Write;
use std::path::Path;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_metadata_domain::{fuse_genres, format_fused_genres};
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
fn test_genre_splitting_deduplication_and_multilingual_preservation() {
    let inputs = [
        "Rock; Pop/Disco",
        "rock",
        "Variété française",
        "Synth-pop/Disco; Pop",
        "Música Latina; Chanson française",
    ];

    let fused = fuse_genres(&inputs);
    // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
    // S184 canonicalization matrix: Synth-pop(2)/Synthpop(8) fuse to audited winner "Synth-Pop"(20).
    assert_eq!(
        fused,
        vec![
            "Rock".to_string(),
            "Pop".to_string(),
            "Disco".to_string(),
            "Variété française".to_string(),
            "Synth-Pop".to_string(),
            "Música Latina".to_string(),
            "Chanson française".to_string(),
        ]
    );

    let formatted = format_fused_genres(&inputs).unwrap();
    assert_eq!(
        formatted,
        "Rock; Pop; Disco; Variété française; Synth-Pop; Música Latina; Chanson française"
    );
}

#[tokio::test]
async fn test_exhaustive_enrichment_genre_multi_provider_collection() {
    let engine = EnrichmentEngine::new();

    let qobuz_source = OriginTrackMetadata {
        title: Some("La Foule".to_string()),
        artist: Some("Édith Piaf".to_string()),
        album: Some("L'Essentiel".to_string()),
        genre: Some("Chanson française; Valse".to_string()),
        source_name: "qobuz".to_string(),
        ..Default::default()
    };

    let tidal_source = OriginTrackMetadata {
        title: Some("La Foule".to_string()),
        artist: Some("Édith Piaf".to_string()),
        album: Some("L'Essentiel".to_string()),
        genre: Some("World/French Pop".to_string()),
        source_name: "tidal".to_string(),
        ..Default::default()
    };

    let spotify_source = OriginTrackMetadata {
        title: Some("La Foule".to_string()),
        artist: Some("Édith Piaf".to_string()),
        album: Some("L'Essentiel".to_string()),
        genre: Some("chanson française; Traditional Pop".to_string()),
        source_name: "spotify".to_string(),
        ..Default::default()
    };

    let enriched = engine.resolve_exhaustive_track_metadata(
        "Édith Piaf",
        "L'Essentiel",
        "La Foule",
        None,
        &[qobuz_source, tidal_source, spotify_source],
        false,
    ).await;

    let expected_genres = [
        "Chanson française",
        "Valse",
        "World",
        "French Pop",
        "Traditional Pop",
    ];

    let enriched_genre_str = enriched.genre.value().unwrap();
    for g in &expected_genres {
        assert!(
            enriched_genre_str.contains(g),
            "Expected fused genre to contain '{}', got '{}'",
            g,
            enriched_genre_str
        );
    }
}

#[test]
fn test_flac_multi_genre_block_writing_and_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("genre_test.flac");
    create_minimal_flac(&flac_path);

    let meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        genre: Some("Art Rock; Glam Rock / Berlin Trilogy; Variété française".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&flac_path, &meta).unwrap();

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();
    let genres = comments.get("GENRE").expect("GENRE tag must exist");

    // Must be written as multiple Vorbis comment blocks
    assert_eq!(
        genres,
        &[
            "Art Rock",
            "Glam Rock",
            "Berlin Trilogy",
            "Variété française"
        ]
    );
}

#[test]
fn test_mp4_multi_genre_tag_writing_and_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let mp4_path = temp_dir.path().join("genre_test.m4a");
    create_minimal_mp4(&mp4_path);

    let meta = Mp4Metadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        genre: Some("Art Rock; Glam Rock / Berlin Trilogy; Variété française".to_string()),
        ..Default::default()
    };

    apply_and_verify_mp4_tags(&mp4_path, &meta).unwrap();

    let tag = mp4ameta::Tag::read_from_path(&mp4_path).unwrap();
    assert_eq!(
        tag.genre().unwrap(),
        "Art Rock; Glam Rock; Berlin Trilogy; Variété française"
    );

    // The freeform ----:com.apple.iTunes:GENRE atom is intentionally NOT written anymore:
    // while both ©gen and the freeform exist, ffmpeg/ffprobe drops the standard lowercase
    // "genre" key regardless of atom order, breaking external-tool parity (see apply_mp4_tags).
    let ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "GENRE");
    assert!(
        tag.strings_of(&ident).next().is_none(),
        "freeform GENRE must be absent; the standard ©gen atom carries the genre"
    );
}
