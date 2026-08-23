//! Physical Tag Inventory Test (S172)
//!
//! Validates physical reading and inspection of tags across:
//! - Qobuz FLAC exact
//! - Tidal FLAC exact
//! - Tidal M4A/AAC quality fallback
//! - Missing-at-source resilience

use std::path::PathBuf;
use syncify_flac_writer::{apply_flac_tags, verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
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

fn create_valid_m4a_dummy(path: &PathBuf) {
    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "anullsrc=r=44100:cl=stereo",
            "-t", "1",
            "-c:a", "aac",
            path.to_str().unwrap(),
        ])
        .output();
}

#[test]
fn test_qobuz_flac_physical_inventory() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("qobuz_track.flac");
    create_valid_flac_dummy(&flac_path);

    let meta = FlacMetadata {
        title: "Comfortably Numb".to_string(),
        artist: "Pink Floyd".to_string(),
        album: "The Wall".to_string(),
        album_artist: Some("Pink Floyd".to_string()),
        track_number: 6,
        track_total: 13,
        disc_number: 2,
        disc_total: 2,
        release_date: Some("1979-11-30".to_string()),
        original_date: Some("1979-11-30".to_string()),
        isrc: Some("GBAYE7900055".to_string()),
        barcode: Some("5099902894423".to_string()),
        catalog_number: Some("SHDW 411".to_string()),
        language: Some("eng".to_string()),
        release_country: Some("US".to_string()),
        genre: Some("Progressive Rock; Art Rock".to_string()),
        bpm: Some(127),
        label: Some("Harvest Records".to_string()),
        composer: Some("David Gilmour; Roger Waters".to_string()),
        performers: Some("Pink Floyd".to_string()),
        lyrics_lrc: Some("[00:01.00]Hello? Is there anybody in there?".to_string()),
        lyrics_source: Some("Musixmatch".to_string()),
        audio_source: Some("Qobuz".to_string()),
        cover_source: Some("Qobuz Cover Art".to_string()),
        copyright: Some("1979 Pink Floyd Music Ltd".to_string()),
        release_region: Some("US".to_string()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).expect("Failed to apply Qobuz FLAC tags");
    verify_flac_tags(&flac_path, &meta).expect("Failed to verify Qobuz FLAC tags");

    // Physical read with metaflac
    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to read with metaflac");
    let vc = tag.vorbis_comments().expect("Missing Vorbis comments");

    assert_eq!(vc.get("TITLE").unwrap()[0], "Comfortably Numb");
    assert_eq!(vc.get("ARTIST").unwrap()[0], "Pink Floyd");
    assert_eq!(vc.get("ALBUM").unwrap()[0], "The Wall");
    assert_eq!(vc.get("TRACKNUMBER").unwrap()[0], "6");
    assert_eq!(vc.get("TRACKTOTAL").unwrap()[0], "13");
    assert_eq!(vc.get("DISCNUMBER").unwrap()[0], "2");
    assert_eq!(vc.get("DISCTOTAL").unwrap()[0], "2");
    assert_eq!(vc.get("ISRC").unwrap()[0], "GBAYE7900055");
    assert_eq!(vc.get("BARCODE").unwrap()[0], "5099902894423");
    assert_eq!(vc.get("UPC").unwrap()[0], "5099902894423");
    assert_eq!(vc.get("LANGUAGE").unwrap()[0], "eng");
    assert_eq!(vc.get("COUNTRY").unwrap()[0], "US");
    assert_eq!(vc.get("RELEASECOUNTRY").unwrap()[0], "US");
    assert_eq!(vc.get("LABEL").unwrap()[0], "Harvest Records");
    assert_eq!(vc.get("RECORDLABEL").unwrap()[0], "Harvest Records");
    assert_eq!(vc.get("ORGANIZATION").unwrap()[0], "Harvest Records");
    assert_eq!(vc.get("BPM").unwrap()[0], "127");
    assert_eq!(vc.get("TEMPO").unwrap()[0], "127");
    assert_eq!(vc.get("SYNCIFY_AUDIO_SOURCE").unwrap()[0], "Qobuz");
}

#[test]
fn test_tidal_flac_physical_inventory() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("tidal_track.flac");
    create_valid_flac_dummy(&flac_path);

    let meta = FlacMetadata {
        title: "11 Besos".to_string(),
        artist: "Morat".to_string(),
        album: "Balas Perdidas".to_string(),
        album_artist: Some("Morat".to_string()),
        track_number: 12,
        track_total: 12,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2018-10-26".to_string()),
        original_date: Some("2018-10-26".to_string()),
        isrc: Some("ES5701800919".to_string()),
        barcode: Some("00602577156156".to_string()),
        language: Some("spa".to_string()),
        release_country: Some("ES".to_string()),
        genre: Some("Latin Pop; Pop Rock".to_string()),
        bpm: Some(95),
        label: Some("Universal Music Spain".to_string()),
        composer: Some("Juan Pablo Isaza; Juan Pablo Villamil".to_string()),
        performers: Some("Morat".to_string()),
        lyrics_lrc: Some("[00:05.00]Con un beso llego la calma".to_string()),
        lyrics_source: Some("Musixmatch".to_string()),
        audio_source: Some("Tidal".to_string()),
        cover_source: Some("Apple Music Animated Cover".to_string()),
        copyright: Some("2018 Universal Music Spain".to_string()),
        release_region: Some("ES".to_string()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).expect("Failed to apply Tidal FLAC tags");
    verify_flac_tags(&flac_path, &meta).expect("Failed to verify Tidal FLAC tags");

    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to read with metaflac");
    let vc = tag.vorbis_comments().expect("Missing Vorbis comments");

    assert_eq!(vc.get("TITLE").unwrap()[0], "11 Besos");
    assert_eq!(vc.get("SYNCIFY_AUDIO_SOURCE").unwrap()[0], "Tidal");
    assert_eq!(vc.get("SYNCIFY_COVER_SOURCE").unwrap()[0], "Apple Music Animated Cover");
    assert_eq!(vc.get("LANGUAGE").unwrap()[0], "spa");
    assert_eq!(vc.get("COUNTRY").unwrap()[0], "ES");
    assert_eq!(vc.get("RECORDLABEL").unwrap()[0], "Universal Music Spain");
}

#[test]
fn test_tidal_m4a_fallback_physical_inventory() {
    let dir = tempdir().unwrap();
    let m4a_path = dir.path().join("06 - #1 Crush.m4a");
    create_valid_m4a_dummy(&m4a_path);

    let mp4_meta = Mp4Metadata {
        title: "#1 Crush".to_string(),
        artist: "Garbage".to_string(),
        album: "Absolute Garbage (Special Edition)".to_string(),
        album_artist: Some("Garbage".to_string()),
        track_number: 6,
        track_total: 18,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        isrc: Some("USNPD0601064".to_string()),
        barcode: Some("5060204805315".to_string()),
        label: Some("Alcopop! Records".to_string()),
        genre: Some("Alternative Rock".to_string()),
        bpm: Some(110),
        language: Some("eng".to_string()),
        release_country: Some("US".to_string()),
        composer: Some("Shirley Manson; Duke Erikson".to_string()),
        performer: Some("Garbage".to_string()),
        lyrics: Some("I would die for you\nI would die for you".to_string()),
        comment: Some("Audio: Tidal Official API | Source: Tidal".to_string()),
        copyright: Some("2024 Garbage".to_string()),
        musicbrainz_track_id: Some("6f5dbcb9-287b-4082-a830-3cf0e6aface9".to_string()),
        musicbrainz_album_id: Some("34559b2a-4343-4ece-9fff-4f5dcfc1fc1b".to_string()),
        musicbrainz_release_group_id: Some("d4253364-ad3f-32bf-9952-d6377b34ec73".to_string()),
        ..Default::default()
    };

    apply_and_verify_mp4_tags(&m4a_path, &mp4_meta).expect("Failed to apply/verify MP4 tags");

    // Physical read with mp4ameta
    let tag = mp4ameta::Tag::read_from_path(&m4a_path).expect("Failed to read M4A tag");
    assert_eq!(tag.title(), Some("#1 Crush"));
    assert_eq!(tag.artist(), Some("Garbage"));
    assert_eq!(tag.album(), Some("Absolute Garbage (Special Edition)"));
    assert_eq!(tag.genre(), Some("Alternative Rock"));
    assert_eq!(tag.bpm(), Some(110));
    assert_eq!(tag.copyright(), Some("2024 Garbage"));
    assert_eq!(tag.track(), (Some(6), Some(18)));
    assert_eq!(tag.disc(), (Some(1), Some(1)));
}

#[test]
fn test_missing_at_source_tolerance() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("minimal_source.flac");
    create_valid_flac_dummy(&flac_path);

    // Track where upstream API has NO composer, NO bpm, NO lyrics, NO language
    let meta = FlacMetadata {
        title: "Ambient Piece".to_string(),
        artist: "Unknown Artist".to_string(),
        album: "Calm Sounds".to_string(),
        album_artist: None,
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2023".to_string()),
        original_date: None,
        isrc: None,
        barcode: None,
        catalog_number: None,
        language: None, // MissingAtSource
        release_country: None, // MissingAtSource
        genre: Some("Ambient".to_string()),
        bpm: None, // MissingAtSource
        label: None, // MissingAtSource
        composer: None, // MissingAtSource
        performers: None,
        lyrics_lrc: None, // MissingAtSource (instrumental)
        lyrics_source: None,
        audio_source: Some("Qobuz".to_string()),
        cover_source: None,
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).expect("MissingAtSource tags must not cause failure");
    verify_flac_tags(&flac_path, &meta).expect("Verification must pass for present fields");
}
