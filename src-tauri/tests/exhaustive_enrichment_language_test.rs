//! S176M Exhaustive Enrichment - Language Resolution & Tag Writing Test Suite
//!
//! Tests:
//! 1. Multi-provider LANGUAGE collection and ISO 639-2 normalization (eng, spa, deu, fra, jpn, etc.).
//! 2. Resolution precedence: StreamingService (Qobuz/Tidal) > MusicBrainz > SpotifyMetadata > Inferred.
//! 3. Majority voting resolution when multiple providers of equal tier disagree.
//! 4. Guarantee that LANGUAGE is never left empty if at least one valid candidate exists.
//! 5. Tag writing and roundtrip verification in FLAC VorbisComments (LANGUAGE) and MP4/M4A standard atom (©lng).

use std::fs::File;
use std::io::Write;
use std::path::Path;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_metadata_domain::{fuse_languages, resolve_language};
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
fn test_iso_639_2_normalization_mappings() {
    assert_eq!(resolve_language("English"), Some("eng".to_string()));
    assert_eq!(resolve_language("en"), Some("eng".to_string()));
    assert_eq!(resolve_language("ENG"), Some("eng".to_string()));

    assert_eq!(resolve_language("Spanish"), Some("spa".to_string()));
    assert_eq!(resolve_language("es"), Some("spa".to_string()));
    assert_eq!(resolve_language("español"), Some("spa".to_string()));

    assert_eq!(resolve_language("German"), Some("deu".to_string()));
    assert_eq!(resolve_language("de"), Some("deu".to_string()));
    assert_eq!(resolve_language("deutsch"), Some("deu".to_string()));

    assert_eq!(resolve_language("French"), Some("fra".to_string()));
    assert_eq!(resolve_language("fr"), Some("fra".to_string()));
    assert_eq!(resolve_language("français"), Some("fra".to_string()));

    assert_eq!(resolve_language("Japanese"), Some("jpn".to_string()));
    assert_eq!(resolve_language("ja"), Some("jpn".to_string()));
    assert_eq!(resolve_language("nihongo"), Some("jpn".to_string()));
}

#[tokio::test]
async fn test_exhaustive_enrichment_language_multi_provider_collection() {
    let engine = EnrichmentEngine::new();

    let qobuz_source = OriginTrackMetadata {
        title: Some("Non, je ne regrette rien".to_string()),
        artist: Some("Édith Piaf".to_string()),
        album: Some("L'Essentiel".to_string()),
        language: Some("French".to_string()),
        source_name: "qobuz".to_string(),
        ..Default::default()
    };

    let tidal_source = OriginTrackMetadata {
        title: Some("Non, je ne regrette rien".to_string()),
        artist: Some("Édith Piaf".to_string()),
        album: Some("L'Essentiel".to_string()),
        language: Some("fra".to_string()),
        source_name: "tidal".to_string(),
        ..Default::default()
    };

    let spotify_source = OriginTrackMetadata {
        title: Some("Non, je ne regrette rien".to_string()),
        artist: Some("Édith Piaf".to_string()),
        album: Some("L'Essentiel".to_string()),
        language: Some("fr".to_string()),
        source_name: "spotify".to_string(),
        ..Default::default()
    };

    let enriched = engine.resolve_exhaustive_track_metadata(
        "Édith Piaf",
        "L'Essentiel",
        "Non, je ne regrette rien",
        None,
        &[qobuz_source, tidal_source, spotify_source],
        false,
    ).await;

    assert_eq!(enriched.language.value(), Some("fra"));
}

#[test]
fn test_language_fusion_precedence_and_majority() {
    // 1. StreamingService (Qobuz) beats Spotify
    let candidates = [
        ("English", "spotify", 0.90),
        ("French", "qobuz", 0.95),
    ];
    let resolved = fuse_languages(&candidates);
    assert_eq!(resolved, Some("fra".to_string()));

    // 2. MusicBrainz beats Spotify
    let mb_candidates = [
        ("English", "spotify", 0.90),
        ("spa", "musicbrainz", 0.85),
    ];
    let resolved_mb = fuse_languages(&mb_candidates);
    assert_eq!(resolved_mb, Some("spa".to_string()));

    // 3. Majority vote across equal tier
    let equal_tier = [
        ("deu", "qobuz", 0.90),
        ("German", "tidal", 0.90),
        ("English", "deezer", 0.90),
    ];
    let resolved_eq = fuse_languages(&equal_tier);
    assert_eq!(resolved_eq, Some("deu".to_string()));

    // 4. Non-empty guarantee: single valid language from any tier is preserved
    let single_spotify = [
        ("Japanese", "spotify", 0.80),
    ];
    let resolved_single = fuse_languages(&single_spotify);
    assert_eq!(resolved_single, Some("jpn".to_string()));
}

#[test]
fn test_flac_language_tag_writing_and_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("language_test.flac");
    create_minimal_flac(&flac_path);

    let meta = FlacMetadata {
        title: "La Vie En Rose".to_string(),
        artist: "Édith Piaf".to_string(),
        album: "Chansons".to_string(),
        language: Some("French".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&flac_path, &meta).unwrap();

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();
    assert_eq!(comments.get("LANGUAGE").unwrap(), &["fra"]);
}

#[test]
fn test_mp4_language_tag_writing_and_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let mp4_path = temp_dir.path().join("language_test.m4a");
    create_minimal_mp4(&mp4_path);

    let meta = Mp4Metadata {
        title: "Despacito".to_string(),
        artist: "Luis Fonsi".to_string(),
        album: "Vida".to_string(),
        language: Some("Spanish".to_string()),
        ..Default::default()
    };

    apply_and_verify_mp4_tags(&mp4_path, &meta).unwrap();

    let tag = mp4ameta::Tag::read_from_path(&mp4_path).unwrap();
    let lang_str = tag.strings_of(&mp4ameta::Fourcc(*b"\xa9lng")).next().unwrap();
    assert_eq!(lang_str, "spa");

    let freeform_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "LANGUAGE");
    assert!(tag.strings_of(&freeform_ident).next().is_none(), "Freeform LANGUAGE atom must be absent");
}
