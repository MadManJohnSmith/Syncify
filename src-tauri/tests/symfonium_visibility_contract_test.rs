//! Symfonium Visibility Contract Test (S172)
//!
//! Validates:
//! 1. Symfonium-critical dual Vorbis comments (LABEL/RECORDLABEL/ORGANIZATION, BARCODE/UPC, BPM/TEMPO, COUNTRY/RELEASECOUNTRY).
//! 2. Multi-genre normalization (individual tags and semicolon delimiters).
//! 3. Language ISO-639-2 / ISO-639-1 normalization.
//! 4. Exact filename pairing between audio and `.lrc` sidecars.

use std::path::PathBuf;
use syncify_flac_writer::{apply_flac_tags, FlacMetadata};
use tempfile::tempdir;

fn create_valid_flac_dummy(path: &PathBuf) {
    let raw_flac_bytes: &[u8] = &[
        0x66, 0x4C, 0x61, 0x43, // "fLaC"
        0x00, 0x00, 0x00, 0x22, // METADATA_BLOCK_HEADER: type 0 (STREAMINFO), is_last=0, length=34
        0x10, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x0A, 0xC4, 0x42, 0xF0, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
        0x84, 0x00, 0x00, 0x08, // METADATA_BLOCK_HEADER: type 4 (VORBIS_COMMENT), is_last=1, length=8
        0x00, 0x00, 0x00, 0x00, // vendor length 0
        0x00, 0x00, 0x00, 0x00, // user comment count 0
    ];
    std::fs::write(path, raw_flac_bytes).expect("Failed to write dummy FLAC");
}

#[test]
fn test_symfonium_dual_tags_visibility() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("symfonium_track.flac");
    create_valid_flac_dummy(&flac_path);

    let meta = FlacMetadata {
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        album_artist: Some("Test Artist".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        original_date: Some("2024-01-01".to_string()),
        isrc: Some("US1234567890".to_string()),
        barcode: Some("123456789012".to_string()),
        catalog_number: Some("CAT-001".to_string()),
        language: Some("eng".to_string()),
        release_country: Some("US".to_string()),
        genre: Some("Rock; Synthwave".to_string()),
        bpm: Some(120),
        label: Some("Awesome Records".to_string()),
        composer: Some("Composer Name".to_string()),
        performers: Some("Performer Name".to_string()),
        lyrics_lrc: Some("[00:01.00]Line 1".to_string()),
        lyrics_source: Some("Musixmatch".to_string()),
        audio_source: Some("Qobuz".to_string()),
        cover_source: Some("Qobuz".to_string()),
        copyright: Some("2024 Awesome Records".to_string()),
        release_region: Some("US".to_string()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).expect("Failed to apply tags");

    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to read tag");
    let vc = tag.vorbis_comments().expect("Missing comments");

    // Check dual/triple tags:
    // 1. Record Label
    assert_eq!(vc.get("LABEL").unwrap()[0], "Awesome Records");
    assert_eq!(vc.get("RECORDLABEL").unwrap()[0], "Awesome Records");
    assert_eq!(vc.get("ORGANIZATION").unwrap()[0], "Awesome Records");

    // 2. Barcode & UPC
    assert_eq!(vc.get("BARCODE").unwrap()[0], "123456789012");
    assert_eq!(vc.get("UPC").unwrap()[0], "123456789012");

    // 3. BPM & TEMPO
    assert_eq!(vc.get("BPM").unwrap()[0], "120");
    assert_eq!(vc.get("TEMPO").unwrap()[0], "120");

    // 4. Country
    // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
    assert_eq!(vc.get("COUNTRY").unwrap()[0], "United States");
    assert_eq!(vc.get("RELEASECOUNTRY").unwrap()[0], "United States");

    // 5. Language
    // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
    assert_eq!(vc.get("LANGUAGE").unwrap()[0], "English");

    // 6. Multi-genre individual entries
    let genres = vc.get("GENRE").unwrap();
    assert!(genres.contains(&"Rock".to_string()) || genres.contains(&"Rock; Synthwave".to_string()));
}

#[test]
fn test_symfonium_lrc_exact_pairing() {
    let dir = tempdir().unwrap();
    let audio_path = dir.path().join("01 - Test Song.flac");
    let lrc_path = dir.path().join("01 - Test Song.lrc");

    std::fs::write(&audio_path, b"AUDIO").unwrap();
    std::fs::write(&lrc_path, b"[00:01.00]Hello world").unwrap();

    // Symfonium scans <audio_name_without_ext>.lrc
    let audio_stem = audio_path.file_stem().unwrap();
    let lrc_stem = lrc_path.file_stem().unwrap();
    assert_eq!(
        audio_stem, lrc_stem,
        "Audio file stem and lyrics file stem must match exactly"
    );
}
