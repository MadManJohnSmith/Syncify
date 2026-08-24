//! S178 Facet Tags Roundtrip Test Suite
//!
//! Validates roundtrip persistence and multi-tag/multi-value Vorbis comment writing
//! for all Symfonium-confirmed facet tags:
//! - LANGUAGE (English, eng)
//! - STYLE, ALBUMSTYLE, TRACKSTYLE
//! - MOOD, ALBUMMOOD, TRACKMOOD
//! - TAGS, ALBUMTAGS

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
fn test_style_mood_tags_language_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("facet_track.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Enjoy the Silence".to_string(),
        artist: "Depeche Mode".to_string(),
        album: "Violator".to_string(),
        genre: Some("Synth-Pop; Electronic".to_string()),
        style: Some("New Wave; Darkwave".to_string()),
        mood: Some("Melancholic; Atmospheric".to_string()),
        tags: Some("80s Classics; Synthpop Legends".to_string()),
        language: Some("English".to_string()),
        grouping: Some("Depeche Mode - Violator".to_string()),
        track_number: 1,
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write");
    assert!(report.tags_match);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read flac tags");
    let comments = tag_obj.vorbis_comments().expect("vorbis comments");

    // Check LANGUAGE (wire format carries the English display name)
    // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
    let lang = comments.get("LANGUAGE").expect("LANGUAGE tag");
    assert_eq!(lang, &["English".to_string()]);

    // Check STYLE / ALBUMSTYLE / TRACKSTYLE
    // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
    // S184 canonicalization matrix: "Darkwave"(2) fuses to audited winner "Dark Wave"(3).
    let style = comments.get("STYLE").expect("STYLE tag");
    assert_eq!(style, &["New Wave".to_string(), "Dark Wave".to_string()]);
    let album_style = comments.get("ALBUMSTYLE").expect("ALBUMSTYLE tag");
    assert_eq!(album_style, &["New Wave".to_string(), "Dark Wave".to_string()]);
    let track_style = comments.get("TRACKSTYLE").expect("TRACKSTYLE tag");
    assert_eq!(track_style, &["New Wave".to_string(), "Dark Wave".to_string()]);

    // Check MOOD / ALBUMMOOD / TRACKMOOD
    let mood = comments.get("MOOD").expect("MOOD tag");
    assert_eq!(mood, &["Melancholic".to_string(), "Atmospheric".to_string()]);
    let album_mood = comments.get("ALBUMMOOD").expect("ALBUMMOOD tag");
    assert_eq!(album_mood, &["Melancholic".to_string(), "Atmospheric".to_string()]);
    let track_mood = comments.get("TRACKMOOD").expect("TRACKMOOD tag");
    assert_eq!(track_mood, &["Melancholic".to_string(), "Atmospheric".to_string()]);

    // Check TAGS / ALBUMTAGS
    let tags = comments.get("TAGS").expect("TAGS tag");
    assert_eq!(tags, &["80s Classics".to_string(), "Synthpop Legends".to_string()]);
    let album_tags = comments.get("ALBUMTAGS").expect("ALBUMTAGS tag");
    assert_eq!(album_tags, &["80s Classics".to_string(), "Synthpop Legends".to_string()]);
}
