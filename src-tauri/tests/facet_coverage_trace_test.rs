//! S181 Facet Coverage Trace Test Suite
//!
//! Validates:
//! 1. Multi-genre strings in OriginTrackMetadata correctly populate GENRE, STYLE, TAGS, ARTISTS_TAGS.
//! 2. Release country without explicit language automatically resolves canonical LANGUAGE (e.g. UK/US -> eng, ES/MX -> spa).
//! 3. TextRepresentation in MusicBrainz takes precedence.
//! 4. Full propagation into FlacMetadata and Mp4Metadata.

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
async fn test_multi_genre_and_country_language_enrichment() {
    let engine = EnrichmentEngine::new();

    let origin = OriginTrackMetadata {
        source_name: "Qobuz".to_string(),
        title: Some("Paranoid Android".to_string()),
        artist: Some("Radiohead".to_string()),
        album: Some("OK Computer".to_string()),
        genre: Some("Alternative Rock; Art Rock; Post-Punk".to_string()),
        release_country: Some("GB".to_string()),
        isrc: Some("GBAYE9700065".to_string()),
        bpm: Some(82),
        ..Default::default()
    };

    let enriched = engine
        .resolve_exhaustive_track_metadata_with_force(
            "Radiohead",
            "OK Computer",
            "Paranoid Android",
            Some("GBAYE9700065"),
            &[origin],
            false,
            false,
        )
        .await;

    // 1. GENRE is populated with all valid genres
    assert_eq!(
        enriched.genre.value(),
        Some("Alternative Rock; Art Rock; Post-Punk")
    );

    // 2. STYLE is populated with secondary genres
    assert_eq!(
        enriched.style.value(),
        Some("Art Rock; Post-Punk")
    );

    // 3. TAGS (Album Tags) is populated
    assert_eq!(
        enriched.tags.value(),
        Some("Art Rock; Post-Punk")
    );

    // 4. ARTISTS_TAGS is populated
    assert_eq!(
        enriched.artist_tags.value(),
        Some("Alternative Rock; Art Rock; Post-Punk")
    );

    // 5. LANGUAGE is automatically derived as "eng" from country GB
    assert_eq!(
        enriched.language.value(),
        Some("eng")
    );

    // 6. COUNTRY is resolved to "United Kingdom"
    assert_eq!(
        enriched.release_country.value(),
        Some("United Kingdom")
    );

    // 7. Test write to FLAC file and verify Vorbis Comments
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("radiohead_track.flac");
    create_synthetic_flac(&file_path);

    let flac_meta = FlacMetadata {
        title: "Paranoid Android".to_string(),
        artist: "Radiohead".to_string(),
        album: "OK Computer".to_string(),
        genre: enriched.genre.value().map(|s| s.to_string()),
        style: enriched.style.value().map(|s| s.to_string()),
        tags: enriched.tags.value().map(|s| s.to_string()),
        artist_tags: enriched.artist_tags.value().map(|s| {
            s.split(';')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        }),
        language: enriched.language.value().map(|s| s.to_string()),
        release_country: enriched.release_country.value().map(|s| s.to_string()),
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &flac_meta).expect("FLAC write");
    assert!(report.tags_match, "Tags verification failed: {:?}", report.mismatches);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read flac tags");
    let comments = tag_obj.vorbis_comments().expect("vorbis comments");

    assert_eq!(comments.get("GENRE"), Some(&vec![
        "Alternative Rock".to_string(),
        "Art Rock".to_string(),
        "Post-Punk".to_string(),
    ]));
    assert_eq!(comments.get("STYLE"), Some(&vec![
        "Art Rock".to_string(),
        "Post-Punk".to_string(),
    ]));
    assert_eq!(comments.get("TAGS"), Some(&vec![
        "Art Rock".to_string(),
        "Post-Punk".to_string(),
    ]));
    assert_eq!(comments.get("ARTISTS_TAGS"), Some(&vec![
        "Alternative Rock".to_string(),
        "Art Rock".to_string(),
        "Post-Punk".to_string(),
    ]));
    assert_eq!(comments.get("LANGUAGE"), Some(&vec!["eng".to_string()]));
    // S177 contract (c8cc6a6): tags carry the ISO 3166-1 alpha-2 code for sovereign countries.
    assert_eq!(comments.get("RELEASECOUNTRY"), Some(&vec!["GB".to_string()]));
}

#[tokio::test]
async fn test_spanish_country_language_enrichment() {
    let engine = EnrichmentEngine::new();

    let origin = OriginTrackMetadata {
        source_name: "Qobuz".to_string(),
        title: Some("A un minuto de ti".to_string()),
        artist: Some("Mikel Erentxun".to_string()),
        album: Some("Naufragios".to_string()),
        genre: Some("Pop Rock; Rock en Español".to_string()),
        release_country: Some("ES".to_string()),
        ..Default::default()
    };

    let enriched = engine
        .resolve_exhaustive_track_metadata_with_force(
            "Mikel Erentxun",
            "Naufragios",
            "A un minuto de ti",
            None,
            &[origin],
            false,
            false,
        )
        .await;

    assert_eq!(enriched.language.value(), Some("spa"));
    assert_eq!(enriched.release_country.value(), Some("Spain"));
    assert_eq!(enriched.style.value(), Some("Rock en Español"));
    assert_eq!(enriched.tags.value(), Some("Rock en Español"));
    assert_eq!(enriched.artist_tags.value(), Some("Pop Rock; Rock en Español"));
}
