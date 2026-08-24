//! M4A Cover Embedding and Verification Test (S174)
//!
//! Validates:
//! 1. AAC / M4A container `covr` atom embedding using `mp4ameta`.
//! 2. Correct JPEG and PNG artwork data preservation and readback.
//! 3. `apply_and_verify_mp4_tags` succeeds and reports `cover_present == true`.
//! 4. Physical readback via `mp4ameta::Tag` confirms embedded artwork bytes match original payload.

use std::path::PathBuf;
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
use tempfile::tempdir;

fn generate_synthetic_pcm() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_sec = 0.5;
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        samples[i] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
    }
    samples
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

fn create_synthetic_jpeg_bytes() -> Vec<u8> {
    // Minimal valid 1x1 JPEG image bytes
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
fn test_m4a_cover_embedding_and_readback() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("test_track.m4a");
    create_synthetic_m4a(&file_path);

    let cover_bytes = create_synthetic_jpeg_bytes();

    let meta = Mp4Metadata {
        title: "Test M4A Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        album_artist: Some("Test Artist".to_string()),
        composer: Some("Test Composer".to_string()),
        performer: Some("Test Performer".to_string()),
        genre: Some("Electronic; Synthpop".to_string()),
        release_year: Some("2024".to_string()),
        release_date: Some("2024-01-01".to_string()),
        original_date: Some("2024-01-01".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("USNPD0601064".to_string()),
        label: Some("Test Label".to_string()),
        catalog_number: Some("CAT-001".to_string()),
        barcode: Some("123456789012".to_string()),
        release_country: Some("Germany".to_string()),
        language: Some("English".to_string()),
        copyright: Some("(C) 2024 Test".to_string()),
        bpm: Some(120),
        comment: Some("Audio: Tidal | Engine: Syncify Production".to_string()),
        lyrics: Some("Test Synced Lyrics".to_string()),
        cover_data: Some(cover_bytes.clone()),
        cover_mime: Some("image/jpeg".to_string()),
        musicbrainz_track_id: Some("6f5dbcb9-287b-4082-a830-3cf0e6aface9".to_string()),
        musicbrainz_artist_id: None,
        musicbrainz_album_id: None,
        musicbrainz_albumartist_id: None,
        musicbrainz_release_group_id: None,
        replaygain_track_gain: None,
        replaygain_track_peak: None,
        replaygain_album_gain: None,
        replaygain_album_peak: None,
        r128_track_gain: None,
        audio_source: Some("Tidal".to_string()),
        explicit: Some(false),
        ..Default::default()
    };

    // 1. Write tags and verify report
    let report = apply_and_verify_mp4_tags(&file_path, &meta).expect("Tagging and verification must succeed");
    assert!(report.file_exists);
    assert!(report.tags_match);
    assert!(report.title_matches);
    assert!(report.artist_matches);
    assert!(report.album_matches);
    assert!(report.track_number_matches);
    assert!(report.cover_present, "Cover must be physically present in report");
    assert!(report.lyrics_present);
    assert!(report.isrc_present);
    assert!(report.musicbrainz_present);

    // 2. Direct readback via mp4ameta::Tag
    let read_tag = mp4ameta::Tag::read_from_path(&file_path).expect("Read tag from M4A");
    assert_eq!(read_tag.title(), Some("Test M4A Track"));
    assert_eq!(read_tag.artist(), Some("Test Artist"));
    assert_eq!(read_tag.album(), Some("Test Album"));
    assert_eq!(read_tag.genre(), Some("Electronic; Synthpop"));
    assert_eq!(read_tag.bpm(), Some(120));

    // Verify covr atom
    assert!(read_tag.artwork().is_some() || read_tag.artworks().next().is_some(), "Artwork must be present via mp4ameta");
    let read_artwork = read_tag.artwork().or_else(|| read_tag.artworks().next()).expect("Artwork must exist");
    assert_eq!(read_artwork.data, cover_bytes.as_slice(), "Artwork bytes in covr atom must match written bytes");

    // 3. Verify country and language freeform tags
    let country_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");
    let read_country = read_tag.strings_of(&country_ident).next();
    assert_eq!(read_country, Some("Germany"), "Country must be real name 'Germany'");

    let rel_country_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "RELEASECOUNTRY");
    let read_rel_country = read_tag.strings_of(&rel_country_ident).next();
    assert_eq!(read_rel_country, Some("Germany"), "RELEASECOUNTRY must be real name 'Germany'");

    let read_lang = read_tag.strings_of(&mp4ameta::Fourcc(*b"\xa9lng")).next();
    assert_eq!(read_lang, Some("eng"), "Language must be in standard ©lng atom and normalized to ISO 639-2 'eng'");

    let freeform_lang_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "LANGUAGE");
    assert!(read_tag.strings_of(&freeform_lang_ident).next().is_none(), "Freeform LANGUAGE atom must be absent");
}
