//! Language Tag Roundtrip & Normalization Test (S174)
//!
//! Validates:
//! 1. `LANGUAGE` normalized to ISO 639-2 (3-letter) across various inputs (English -> eng, Spanish -> spa, Deutsch -> deu, French -> fra, Japanese -> jpn).
//! 2. FLAC container Vorbis comment `LANGUAGE` roundtrip writing and physical readback (exact key `LANGUAGE`, ISO 639-2).
//! 3. M4A / AAC container standard `©lng` atom (`Fourcc(*b"\xa9lng")`) roundtrip writing and physical readback.
//! 4. Strict exclusion of freeform `----:com.apple.iTunes:LANGUAGE` in M4A containers for full Symfonium compatibility.

use mp4ameta::{Fourcc, FreeformIdent, Tag};
use std::path::PathBuf;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_metadata_domain::resolve_language;
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
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
fn test_language_normalization_domain_rules() {
    assert_eq!(resolve_language("English"), Some("eng".to_string()));
    assert_eq!(resolve_language("english"), Some("eng".to_string()));
    assert_eq!(resolve_language("en"), Some("eng".to_string()));
    assert_eq!(resolve_language("eng"), Some("eng".to_string()));

    assert_eq!(resolve_language("Spanish"), Some("spa".to_string()));
    assert_eq!(resolve_language("Español"), Some("spa".to_string()));
    assert_eq!(resolve_language("es"), Some("spa".to_string()));
    assert_eq!(resolve_language("spa"), Some("spa".to_string()));

    assert_eq!(resolve_language("German"), Some("deu".to_string()));
    assert_eq!(resolve_language("deutsch"), Some("deu".to_string()));
    assert_eq!(resolve_language("de"), Some("deu".to_string()));
    assert_eq!(resolve_language("deu"), Some("deu".to_string()));
    assert_eq!(resolve_language("ger"), Some("deu".to_string()));

    assert_eq!(resolve_language("French"), Some("fra".to_string()));
    assert_eq!(resolve_language("français"), Some("fra".to_string()));
    assert_eq!(resolve_language("fr"), Some("fra".to_string()));
    assert_eq!(resolve_language("fra"), Some("fra".to_string()));
    assert_eq!(resolve_language("fre"), Some("fra".to_string()));

    assert_eq!(resolve_language("Japanese"), Some("jpn".to_string()));
    assert_eq!(resolve_language("japones"), Some("jpn".to_string()));
    assert_eq!(resolve_language("ja"), Some("jpn".to_string()));
    assert_eq!(resolve_language("jpn"), Some("jpn".to_string()));
    assert_eq!(resolve_language("nihongo"), Some("jpn".to_string()));

    assert_eq!(resolve_language("Korean"), Some("kor".to_string()));
    assert_eq!(resolve_language("ko"), Some("kor".to_string()));
    assert_eq!(resolve_language("kor"), Some("kor".to_string()));

    assert_eq!(resolve_language("Instrumental"), Some("zxx".to_string()));
    assert_eq!(resolve_language("zxx"), Some("zxx".to_string()));

    assert_eq!(resolve_language(""), None);
    assert_eq!(resolve_language("   "), None);
    assert_eq!(resolve_language("invalid_long_string_not_a_language"), None);
    assert_eq!(resolve_language("123"), None);
    assert_eq!(resolve_language("xx"), None);
}

#[test]
fn test_flac_language_tag_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("flac_lang_test.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Language Test Track".to_string(),
        artist: "Language Artist".to_string(),
        album: "Language Album".to_string(),
        language: Some("Spanish".to_string()),
        genre: Some("Pop".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        isrc: Some("USNPD0601064".to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC tag write & verify");
    assert!(report.tags_match);

    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC tag");
    let vorbis = read_tag.vorbis_comments().expect("Vorbis comments");
    let lang_tags = vorbis.get("LANGUAGE").expect("LANGUAGE comment present");
    assert_eq!(lang_tags.first().map(|s| s.as_str()), Some("spa"), "FLAC Vorbis comment must be named LANGUAGE and value must be 'spa'");
}

#[test]
fn test_m4a_language_tag_roundtrip_standard_clng_atom() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("m4a_lang_test.m4a");
    create_synthetic_m4a(&file_path);

    let meta = Mp4Metadata {
        title: "M4A Language Test".to_string(),
        artist: "M4A Artist".to_string(),
        album: "M4A Album".to_string(),
        language: Some("Deutsch".to_string()),
        genre: Some("Pop".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        isrc: Some("USNPD0601064".to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_mp4_tags(&file_path, &meta).expect("M4A tag write & verify");
    assert!(report.tags_match);

    let read_tag = Tag::read_from_path(&file_path).expect("Read M4A tag");

    // 1. Must be written to standard iTunes ©lng atom (Fourcc \xa9lng)
    let read_lang = read_tag.strings_of(&Fourcc(*b"\xa9lng")).next();
    assert_eq!(read_lang, Some("deu"), "M4A standard atom ©lng must be 'deu'");

    // 2. Must NEVER be written to freeform ----:com.apple.iTunes:LANGUAGE
    let freeform_lang_ident = FreeformIdent::new_static("com.apple.iTunes", "LANGUAGE");
    let freeform_read = read_tag.strings_of(&freeform_lang_ident).next();
    assert_eq!(freeform_read, None, "M4A must NOT contain freeform ----:com.apple.iTunes:LANGUAGE atom");
}

#[test]
fn test_m4a_language_roundtrip_multiple_iso_languages() {
    let test_cases = [
        ("English", "eng"),
        ("Español", "spa"),
        ("French", "fra"),
        ("Japanese", "jpn"),
    ];

    for (input_lang, expected_iso2) in test_cases {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join(format!("m4a_lang_{}.m4a", expected_iso2));
        create_synthetic_m4a(&file_path);

        let meta = Mp4Metadata {
            title: format!("Test Track {}", expected_iso2),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            language: Some(input_lang.to_string()),
            ..Default::default()
        };

        apply_and_verify_mp4_tags(&file_path, &meta).expect("apply_and_verify_mp4_tags must succeed");

        let read_tag = Tag::read_from_path(&file_path).expect("Read M4A tag");
        let read_lang = read_tag.strings_of(&Fourcc(*b"\xa9lng")).next();
        assert_eq!(read_lang, Some(expected_iso2), "M4A atom ©lng must match expected ISO 639-2 code");

        let freeform_lang_ident = FreeformIdent::new_static("com.apple.iTunes", "LANGUAGE");
        assert!(read_tag.strings_of(&freeform_lang_ident).next().is_none(), "Freeform LANGUAGE atom must be absent");
    }
}
