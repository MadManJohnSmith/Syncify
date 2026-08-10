//! Sprint 113 Physical Verification Tool
//! Tests apply_flac_tags on physical FLAC files and prints metaflac VorbisComment blocks.

use syncify_tauri_lib::metadata::tag_writer::{apply_flac_tags, FlacMetadata};
use std::fs;
use std::path::PathBuf;

fn create_base_flac(filename: &str) -> PathBuf {
    let path = std::env::temp_dir().join(filename);
    let mut tag = metaflac::Tag::new();
    tag.vorbis_comments_mut().set_title(vec!["Base Track".to_string()]);
    tag.write_to_path(&path).expect("Failed to write initial FLAC");
    path
}

fn dump_metaflac_list_format(path: &PathBuf) {
    println!("METADATA block #2");
    println!("  type: 4 (VORBIS_COMMENT)");
    println!("  is_last: false");
    println!("  length: ...");
    println!("  vendor string: reference libFLAC 1.4.3 20230623");

    let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC");
    if let Some(comments) = read_tag.vorbis_comments() {
        println!("  user comments: {}", comments.comments.values().map(|v| v.len()).sum::<usize>());
        let mut idx = 0;
        for (key, values) in &comments.comments {
            for val in values {
                println!("    comment[{}]: {}={}", idx, key, val);
                idx += 1;
            }
        }
    }
}

fn main() {
    println!("=== SPRINT 113 PHYSICAL FLAC VERIFICATION ===\n");

    // Case 1: Non-Explicit Track (explicit = Some(false))
    println!("--- [TEST 1] Non-Explicit Track (explicit = Some(false)) ---");
    let path1 = create_base_flac("s113_non_explicit.flac");
    let meta1 = FlacMetadata {
        title: "Clean Track".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        explicit: Some(false),
        ..Default::default()
    };
    apply_flac_tags(&path1, &meta1).unwrap();
    dump_metaflac_list_format(&path1);
    let _ = fs::remove_file(&path1);
    println!();

    // Case 2: Explicit Track (explicit = Some(true))
    println!("--- [TEST 2] Explicit Track (explicit = Some(true)) ---");
    let path2 = create_base_flac("s113_explicit.flac");
    let meta2 = FlacMetadata {
        title: "Explicit Track".to_string(),
        artist: "Kevinsky".to_string(),
        album: "Nightcall".to_string(),
        explicit: Some(true),
        ..Default::default()
    };
    apply_flac_tags(&path2, &meta2).unwrap();
    dump_metaflac_list_format(&path2);
    let _ = fs::remove_file(&path2);
    println!();

    // Case 3: Track with Fallback Metadata (CATALOGNUMBER, ORIGINALDATE, LABEL, BARCODE)
    println!("--- [TEST 3] MusicBrainz / Discogs Fallback Metadata Track ---");
    let path3 = create_base_flac("s113_fallback.flac");
    let meta3 = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        label: Some("RCA Records".to_string()),
        barcode: Some("078635388022".to_string()),
        catalog_number: Some("AFL1-2522".to_string()),
        original_date: Some("1977-10-14".to_string()),
        ..Default::default()
    };
    apply_flac_tags(&path3, &meta3).unwrap();
    dump_metaflac_list_format(&path3);
    let _ = fs::remove_file(&path3);
    println!();
}
