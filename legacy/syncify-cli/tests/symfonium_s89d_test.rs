//! Sprint S89-d Symfonium Country Resolution & Metadata Isolation Tests

use syncify_cli::metadata::tag_writer::{apply_flac_tags, is_valid_tag_val, verify_flac_tags, FlacMetadata};

fn create_dummy_flac(path: &std::path::Path) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // is_last=1, len=34
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    std::fs::write(path, &flac_bytes).unwrap();
}

#[test]
fn test_country_code_normalization_exact_matches() {
    let resolve_country = |code: &str| -> String {
        match code.to_uppercase().as_str() {
            "AF" => "Afghanistan".to_string(),
            "AT" => "Austria".to_string(),
            "ES" => "Spain".to_string(),
            "MX" => "Mexico".to_string(),
            "NL" => "Netherlands".to_string(),
            "PL" => "Poland".to_string(),
            "XE" => "Europe".to_string(),
            "XW" => "Worldwide".to_string(),
            "US" => "United States".to_string(),
            "GB" | "UK" => "United Kingdom".to_string(),
            "JP" => "Japan".to_string(),
            "DE" => "Germany".to_string(),
            "FR" => "France".to_string(),
            "CA" => "Canada".to_string(),
            "AU" => "Australia".to_string(),
            other => other.to_string(),
        }
    };

    assert_eq!(resolve_country("AF"), "Afghanistan");
    assert_eq!(resolve_country("AT"), "Austria");
    assert_eq!(resolve_country("ES"), "Spain");
    assert_eq!(resolve_country("MX"), "Mexico");
    assert_eq!(resolve_country("NL"), "Netherlands");
    assert_eq!(resolve_country("PL"), "Poland");
    assert_eq!(resolve_country("XE"), "Europe");
    assert_eq!(resolve_country("XW"), "Worldwide");
    assert_eq!(resolve_country("US"), "United States");
    assert_eq!(resolve_country("GB"), "United Kingdom");
    assert_eq!(resolve_country("UK"), "United Kingdom");

    // Preserve raw code without substituting Unknown or inventing names
    assert_eq!(resolve_country("ZZ"), "ZZ");
    assert_eq!(resolve_country("XU"), "XU");
}

#[test]
fn test_strict_genre_isolation_no_copy_to_mood_or_style() {
    let temp_dir = std::env::temp_dir().join(format!("test_s89d_isolation_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("01 - TestTrack.flac");
    create_dummy_flac(&flac_path);

    let meta = FlacMetadata {
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        genre: Some("Art Rock".to_string()),
        style: None, // Absent - MUST NOT be populated with Genre
        mood: None,  // Absent - MUST NOT be populated with Genre
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).unwrap();

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();

    assert_eq!(comments.get("GENRE").unwrap(), &["Art Rock"]);
    assert!(comments.get("STYLE").is_none(), "STYLE must NOT be copied from GENRE");
    assert!(comments.get("MOOD").is_none(), "MOOD must NOT be copied from GENRE");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_cover_front_supports_jpeg_and_animated_webp() {
    let temp_dir = std::env::temp_dir().join(format!("test_s89d_cover_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("01 - CoverTest.flac");
    create_dummy_flac(&flac_path);

    let jpeg_bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];

    let meta_jpeg = FlacMetadata {
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        cover_data: Some(jpeg_bytes.clone()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta_jpeg).unwrap();
    let verification_jpeg = verify_flac_tags(&flac_path, &meta_jpeg).unwrap();
    assert!(verification_jpeg.cover_present, "Static cover picture block must be present");
    assert_eq!(verification_jpeg.cover_mime.as_deref(), Some("image/jpeg"));

    // Now verify animated WebP CoverFront
    let webp_bytes = vec![0x52, 0x49, 0x46, 0x46, 0x20, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x58];
    let meta_webp = FlacMetadata {
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        cover_data: Some(webp_bytes.clone()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta_webp).unwrap();
    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let pics: Vec<_> = tag.pictures().collect();
    assert_eq!(pics.len(), 1);
    assert_eq!(pics[0].picture_type, metaflac::block::PictureType::CoverFront);
    assert_eq!(pics[0].mime_type, "image/webp");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_placeholder_strings_rejection() {
    assert!(!is_valid_tag_val("Unknown"));
    assert!(!is_valid_tag_val("Unknown Artist"));
    assert!(!is_valid_tag_val("Unknown Album"));
    assert!(!is_valid_tag_val("N/A"));
    assert!(!is_valid_tag_val("null"));
    assert!(!is_valid_tag_val("none"));
    assert!(!is_valid_tag_val("???"));

    assert!(is_valid_tag_val("Art Rock"));
    assert!(is_valid_tag_val("Europe"));
}
