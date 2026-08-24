//! S179 Media Type & MusicBrainz Secondary Types Test Suite
//!
//! Validates:
//! 1. Derivation and resolution of `media_type` from metadata / MusicBrainz.
//! 2. Ingestion of `MEDIA` and `MUSICTYPE` in FLAC comments.
//! 3. Standard albums without secondary types do NOT produce `MEDIA` / `MUSICTYPE`.

use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use tempfile::tempdir;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

#[tokio::test]
async fn test_media_type_enrichment_and_flac_writing() {
    let service = EnrichmentEngine::new();

    let soundtrack_origin = OriginTrackMetadata {
        source_name: "musicbrainz".to_string(),
        title: Some("Main Titles".to_string()),
        artist: Some("Hans Zimmer".to_string()),
        album: Some("Interstellar (Original Motion Picture Soundtrack)".to_string()),
        media_type: Some("Soundtrack".to_string()),
        track_number: Some(1),
        track_total: Some(16),
        disc_number: Some(1),
        disc_total: Some(1),
        release_year: Some("2014".to_string()),
        ..Default::default()
    };

    let enriched = service
        .resolve_exhaustive_track_metadata_with_force(
            "Hans Zimmer",
            "Interstellar (Original Motion Picture Soundtrack)",
            "Main Titles",
            None,
            &[soundtrack_origin],
            false,
            false,
        )
        .await;

    assert_eq!(enriched.media_type.value(), Some("Soundtrack"));

    // Write to FLAC and verify MEDIA and MUSICTYPE comments
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("soundtrack_track.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Main Titles".to_string(),
        artist: "Hans Zimmer".to_string(),
        album: "Interstellar (Original Motion Picture Soundtrack)".to_string(),
        track_number: 1,
        media_type: enriched.media_type.value().map(|s| s.to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write");
    assert!(report.tags_match, "Tags verification must succeed: {:?}", report.mismatches);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read flac tags");
    let comments = tag_obj.vorbis_comments().expect("vorbis comments");

    assert_eq!(comments.get("MEDIA"), Some(&vec!["Soundtrack".to_string()]));
    assert_eq!(comments.get("MUSICTYPE"), Some(&vec!["Soundtrack".to_string()]));
}

#[test]
fn test_standard_album_produces_no_media_type() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("standard_album_track.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Paranoid Android".to_string(),
        artist: "Radiohead".to_string(),
        album: "OK Computer".to_string(),
        track_number: 2,
        media_type: None,
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write");
    assert!(report.tags_match, "Tags verification must succeed: {:?}", report.mismatches);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read flac tags");
    let comments = tag_obj.vorbis_comments().expect("vorbis comments");

    assert!(comments.get("MEDIA").is_none(), "MEDIA must be absent for standard album");
    assert!(comments.get("MUSICTYPE").is_none(), "MUSICTYPE must be absent for standard album");
}
